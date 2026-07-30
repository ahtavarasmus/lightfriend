use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use backend::utils::tuwunel_history_prune::{
    apply_state_diff_chain, is_historical_state_candidate, parse_state_diff,
    parse_state_event_metadata, pdu_count_bytes, protected_auth_closure_allowing_missing,
    short_event_id, timeline_index_key, StateDiff, StateEventMetadata,
};
use rocksdb::{
    ColumnFamilyDescriptor, DBWithThreadMode, IteratorMode, MultiThreaded, Options, WriteBatch,
};
use serde::Serialize;

const REQUIRED_COLUMN_FAMILIES: &[&str] = &[
    "roomid_shortstatehash",
    "shortstatehash_statediff",
    "shorteventid_eventid",
    "pduid_pdu",
    "eventid_pduid",
    "eventid_outlierpdu",
    "eventid_originalpdu",
    "roomid_tscount_pducount",
];
const COMPACTION_COLUMN_FAMILIES: &[&str] = &[
    "pduid_pdu",
    "eventid_pduid",
    "eventid_outlierpdu",
    "eventid_originalpdu",
    "roomid_tscount_pducount",
];
const DEFAULT_RETENTION_SECS: u64 = 60;
const DEFAULT_COMPACTION_SAFETY_BYTES: u64 = 128 * 1024 * 1024;

type RocksDb = DBWithThreadMode<MultiThreaded>;

#[derive(Debug)]
struct TimelineLocation {
    pdu_key: Box<[u8]>,
    index_key: Vec<u8>,
    payload_bytes: u64,
}

#[derive(Debug)]
struct StateEventRecord {
    metadata: StateEventMetadata,
    timeline: Option<TimelineLocation>,
    outlier_payload_bytes: u64,
}

#[derive(Debug, Serialize)]
struct PruneStatus {
    status: &'static str,
    started_at_epoch: u64,
    completed_at_epoch: u64,
    database_path: String,
    cutoff_epoch_millis: u64,
    retention_secs: u64,
    rooms_scanned: u64,
    state_hashes_loaded: u64,
    timeline_pdus_scanned: u64,
    outlier_pdus_scanned: u64,
    state_events_scanned: u64,
    current_state_events_preserved: u64,
    retained_state_events_preserved: u64,
    auth_closure_events_preserved: u64,
    missing_auth_event_payloads: u64,
    dangling_state_events_repaired: u64,
    state_diff_references_removed: u64,
    state_hashes_rewritten: u64,
    historical_state_events_deleted: u64,
    timeline_payloads_deleted: u64,
    outlier_payloads_deleted: u64,
    original_payload_keys_deleted: u64,
    timeline_index_keys_deleted: u64,
    estimated_payload_bytes_deleted: u64,
    database_bytes_before: u64,
    database_bytes_after: u64,
    free_bytes_before_compaction: u64,
    compaction_required_free_bytes: u64,
    compaction_status: &'static str,
}

fn main() -> Result<()> {
    let database_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "/var/lib/tuwunel".to_owned());
    let status_path = env::args()
        .nth(2)
        .unwrap_or_else(|| "/data/seed/tuwunel-history-prune-status.json".to_owned());
    let started_at_epoch = now_epoch_secs()?;

    // Direct RocksDB state-history mutation is recovery tooling, not a safe
    // default startup migration. It must be explicitly enabled after being
    // validated against the exact production Tuwunel release.
    if !env_bool("TUWUNEL_HISTORICAL_STATE_PRUNE_ENABLED", false) {
        write_json_status(
            Path::new(&status_path),
            &serde_json::json!({
                "status": "disabled",
                "started_at_epoch": started_at_epoch,
                "completed_at_epoch": now_epoch_secs()?,
                "database_path": database_path,
            }),
        )?;
        return Ok(());
    }

    if !Path::new(&database_path).join("CURRENT").is_file() {
        write_json_status(
            Path::new(&status_path),
            &serde_json::json!({
                "status": "skipped_empty_database",
                "started_at_epoch": started_at_epoch,
                "completed_at_epoch": now_epoch_secs()?,
                "database_path": database_path,
            }),
        )?;
        return Ok(());
    }

    match prune(Path::new(&database_path), started_at_epoch) {
        Ok(status) => {
            write_json_status(Path::new(&status_path), &status)?;
            println!("{}", serde_json::to_string(&status)?);
            Ok(())
        }
        Err(error) => {
            let failure = serde_json::json!({
                "status": "failed_before_or_during_prune",
                "started_at_epoch": started_at_epoch,
                "completed_at_epoch": now_epoch_secs().unwrap_or(started_at_epoch),
                "database_path": database_path,
                "error": format!("{error:#}"),
            });
            write_json_status(Path::new(&status_path), &failure)?;
            Err(error)
        }
    }
}

fn prune(database_path: &Path, started_at_epoch: u64) -> Result<PruneStatus> {
    let retention_secs = env_u64(
        "TUWUNEL_HISTORICAL_STATE_PRUNE_RETENTION_SECS",
        DEFAULT_RETENTION_SECS,
    );
    let cutoff_epoch_millis = started_at_epoch
        .saturating_sub(retention_secs)
        .saturating_mul(1000);
    let database_bytes_before = directory_size(database_path);
    let db = open_database(database_path)?;

    let current_state_hashes = load_current_state_hashes(&db)?;
    let (mut state_events, timeline_pdus_scanned) = scan_timeline_state_events(&db)?;
    let outlier_pdus_scanned = scan_outlier_state_events(&db, &mut state_events)?;
    let repair = repair_dangling_state_references(&db, &state_events, &current_state_hashes)?;

    let current_short_event_ids = load_current_short_event_ids(&db, &current_state_hashes)?;
    let current_state_event_ids = load_state_event_ids(&db, &current_short_event_ids)?;
    let (retained_short_event_ids, state_hashes_loaded) = load_retained_short_event_ids(&db)?;
    let retained_state_event_ids = load_state_event_ids(&db, &retained_short_event_ids)?;
    let (protected_events, missing_auth_events) = protected_auth_closure_allowing_missing(
        &retained_state_event_ids,
        &metadata_map(&state_events),
    );

    let candidates = if missing_auth_events.is_empty() {
        state_events
            .iter()
            .filter(|(_, record)| {
                is_historical_state_candidate(
                    &record.metadata,
                    &protected_events,
                    cutoff_epoch_millis,
                )
            })
            .map(|(event_id, _)| event_id.clone())
            .collect::<Vec<_>>()
    } else {
        eprintln!(
            "WARNING: {} retained auth-event payloads are already missing; \
             repaired dangling state references but skipped further payload deletion",
            missing_auth_events.len()
        );
        Vec::new()
    };

    let plan = verify_deletion_plan(&db, &state_events, &candidates)?;
    let mut batch = WriteBatch::default();
    apply_deletion_plan(&db, &mut batch, &state_events, &candidates)?;
    db.write(batch)
        .context("failed to atomically delete historical state payloads")?;
    db.flush_wal(true)
        .context("failed to flush historical state payload deletion")?;

    let free_bytes_before_compaction = filesystem_available_bytes(database_path).unwrap_or(0);
    let compaction_safety_bytes = env_u64(
        "TUWUNEL_HISTORICAL_STATE_COMPACTION_SAFETY_BYTES",
        DEFAULT_COMPACTION_SAFETY_BYTES,
    );
    let compaction_required_free_bytes = plan
        .estimated_payload_bytes_deleted
        .saturating_add(compaction_safety_bytes);
    let compaction_status = if candidates.is_empty() {
        "not_needed"
    } else if !env_bool("TUWUNEL_HISTORICAL_STATE_COMPACTION_ENABLED", true) {
        "disabled"
    } else if free_bytes_before_compaction < compaction_required_free_bytes {
        "skipped_insufficient_headroom"
    } else {
        for name in COMPACTION_COLUMN_FAMILIES {
            let cf = required_cf(&db, name)?;
            db.compact_range_cf(&cf, None::<&[u8]>, None::<&[u8]>);
        }
        "completed"
    };

    drop(db);
    let database_bytes_after = directory_size(database_path);

    Ok(PruneStatus {
        status: "success",
        started_at_epoch,
        completed_at_epoch: now_epoch_secs()?,
        database_path: database_path.display().to_string(),
        cutoff_epoch_millis,
        retention_secs,
        rooms_scanned: current_state_hashes.len() as u64,
        state_hashes_loaded,
        timeline_pdus_scanned,
        outlier_pdus_scanned,
        state_events_scanned: state_events.len() as u64,
        current_state_events_preserved: current_state_event_ids.len() as u64,
        retained_state_events_preserved: retained_state_event_ids.len() as u64,
        auth_closure_events_preserved: protected_events
            .len()
            .saturating_sub(retained_state_event_ids.len())
            as u64,
        missing_auth_event_payloads: missing_auth_events.len() as u64,
        dangling_state_events_repaired: repair.dangling_state_events_repaired,
        state_diff_references_removed: repair.state_diff_references_removed,
        state_hashes_rewritten: repair.state_hashes_rewritten,
        historical_state_events_deleted: candidates.len() as u64,
        timeline_payloads_deleted: plan.timeline_payloads_deleted,
        outlier_payloads_deleted: plan.outlier_payloads_deleted,
        original_payload_keys_deleted: plan.original_payload_keys_deleted,
        timeline_index_keys_deleted: plan.timeline_payloads_deleted,
        estimated_payload_bytes_deleted: plan.estimated_payload_bytes_deleted,
        database_bytes_before,
        database_bytes_after,
        free_bytes_before_compaction,
        compaction_required_free_bytes,
        compaction_status,
    })
}

#[derive(Default)]
struct StateReferenceRepair {
    dangling_state_events_repaired: u64,
    state_diff_references_removed: u64,
    state_hashes_rewritten: u64,
}

fn repair_dangling_state_references(
    db: &RocksDb,
    state_events: &HashMap<String, StateEventRecord>,
    current_state_hashes: &[u64],
) -> Result<StateReferenceRepair> {
    let state_diffs_cf = required_cf(db, "shortstatehash_statediff")?;
    let short_events_cf = required_cf(db, "shorteventid_eventid")?;
    let mut diffs = HashMap::<u64, StateDiff>::new();
    let mut short_event_ids = HashSet::new();

    for item in db.iterator_cf(&state_diffs_cf, IteratorMode::Start) {
        let (key, value) = item.context("failed while auditing retained state diffs")?;
        let hash = u64::from_be_bytes(key.as_ref().try_into().with_context(|| {
            format!("state-diff key is not eight bytes: {}", hex::encode(&key))
        })?);
        let diff = parse_state_diff(&value)
            .with_context(|| format!("invalid retained state diff {hash}"))?;
        short_event_ids.extend(diff.added.iter().chain(&diff.removed).map(short_event_id));
        diffs.insert(hash, diff);
    }

    let mut dangling_short_ids = HashSet::new();
    for short_id in short_event_ids {
        let event_id = db
            .get_cf(&short_events_cf, short_id.to_be_bytes())
            .with_context(|| format!("failed to resolve short event id {short_id}"))?
            .with_context(|| format!("short event id {short_id} has no event id"))?;
        let event_id = std::str::from_utf8(&event_id)
            .with_context(|| format!("event id for short id {short_id} is not UTF-8"))?;
        if !state_events.contains_key(event_id) {
            dangling_short_ids.insert(short_id);
        }
    }

    if dangling_short_ids.is_empty() {
        return Ok(StateReferenceRepair::default());
    }

    let mut repair = StateReferenceRepair {
        dangling_state_events_repaired: dangling_short_ids.len() as u64,
        ..StateReferenceRepair::default()
    };
    let mut repaired_diffs = HashMap::with_capacity(diffs.len());
    let mut batch = WriteBatch::default();

    for (hash, mut diff) in diffs {
        let original_count = diff.added.len().saturating_add(diff.removed.len());
        diff.added
            .retain(|compressed| !dangling_short_ids.contains(&short_event_id(compressed)));
        diff.removed
            .retain(|compressed| !dangling_short_ids.contains(&short_event_id(compressed)));
        let repaired_count = diff.added.len().saturating_add(diff.removed.len());
        let removed = original_count.saturating_sub(repaired_count);
        if removed > 0 {
            batch.put_cf(
                &state_diffs_cf,
                hash.to_be_bytes(),
                serialize_state_diff(&diff),
            );
            repair.state_diff_references_removed = repair
                .state_diff_references_removed
                .saturating_add(removed as u64);
            repair.state_hashes_rewritten = repair.state_hashes_rewritten.saturating_add(1);
        }
        repaired_diffs.insert(hash, diff);
    }

    verify_current_state_after_repair(
        current_state_hashes,
        &repaired_diffs,
        &short_events_cf,
        db,
        state_events,
    )?;
    db.write(batch)
        .context("failed to atomically repair dangling state references")?;
    db.flush_wal(true)
        .context("failed to flush dangling state-reference repair")?;

    eprintln!(
        "Repaired {} dangling state events across {} state hashes ({} references removed)",
        repair.dangling_state_events_repaired,
        repair.state_hashes_rewritten,
        repair.state_diff_references_removed
    );
    Ok(repair)
}

fn verify_current_state_after_repair(
    current_state_hashes: &[u64],
    diffs: &HashMap<u64, StateDiff>,
    short_events_cf: &std::sync::Arc<rocksdb::BoundColumnFamily<'_>>,
    db: &RocksDb,
    state_events: &HashMap<String, StateEventRecord>,
) -> Result<()> {
    for current_hash in current_state_hashes {
        let mut chain = Vec::new();
        let mut visited = HashSet::new();
        let mut cursor = Some(*current_hash);
        while let Some(hash) = cursor {
            if !visited.insert(hash) {
                bail!("cycle detected while verifying repaired state at hash {hash}");
            }
            let diff = diffs
                .get(&hash)
                .with_context(|| format!("repaired state diff {hash} is missing"))?;
            cursor = diff.parent;
            chain.push(diff.clone());
        }
        chain.reverse();
        for compressed in apply_state_diff_chain(&chain) {
            let short_id = short_event_id(&compressed);
            let event_id = db
                .get_cf(short_events_cf, short_id.to_be_bytes())
                .with_context(|| format!("failed to verify short event id {short_id}"))?
                .with_context(|| format!("short event id {short_id} has no event id"))?;
            let event_id = std::str::from_utf8(&event_id)
                .with_context(|| format!("event id for short id {short_id} is not UTF-8"))?;
            if !state_events.contains_key(event_id) {
                bail!("current state still references a missing payload: {event_id}");
            }
        }
    }
    Ok(())
}

fn serialize_state_diff(diff: &StateDiff) -> Vec<u8> {
    let separator_bytes = usize::from(!diff.removed.is_empty()) * 8;
    let mut value =
        Vec::with_capacity(8 + (diff.added.len() + diff.removed.len()) * 16 + separator_bytes);
    value.extend_from_slice(&diff.parent.unwrap_or(0).to_be_bytes());
    for compressed in &diff.added {
        value.extend_from_slice(compressed);
    }
    if !diff.removed.is_empty() {
        value.extend_from_slice(&0_u64.to_be_bytes());
        for compressed in &diff.removed {
            value.extend_from_slice(compressed);
        }
    }
    value
}

fn open_database(path: &Path) -> Result<RocksDb> {
    let mut options = Options::default();
    options.create_if_missing(false);
    options.create_missing_column_families(false);
    let names = RocksDb::list_cf(&options, path)
        .with_context(|| format!("failed to list RocksDB columns in {}", path.display()))?;
    for required in REQUIRED_COLUMN_FAMILIES {
        if !names.iter().any(|name| name == required) {
            bail!("required RocksDB column family is missing: {required}");
        }
    }
    let descriptors = names
        .into_iter()
        .map(|name| ColumnFamilyDescriptor::new(name, Options::default()));
    RocksDb::open_cf_descriptors(&options, path, descriptors)
        .with_context(|| format!("failed to open Tuwunel RocksDB at {}", path.display()))
}

fn load_current_state_hashes(db: &RocksDb) -> Result<Vec<u64>> {
    let cf = required_cf(db, "roomid_shortstatehash")?;
    db.iterator_cf(&cf, IteratorMode::Start)
        .map(|item| {
            let (_, value) = item.context("failed to read room current-state hash")?;
            let bytes: [u8; 8] = value
                .as_ref()
                .try_into()
                .context("room current-state hash is not eight bytes")?;
            Ok(u64::from_be_bytes(bytes))
        })
        .collect()
}

fn load_current_short_event_ids(
    db: &RocksDb,
    current_state_hashes: &[u64],
) -> Result<HashSet<u64>> {
    let cf = required_cf(db, "shortstatehash_statediff")?;
    let mut event_ids = HashSet::new();

    for current_hash in current_state_hashes {
        let mut chain = Vec::<StateDiff>::new();
        let mut chain_hashes = HashSet::new();
        let mut cursor = Some(*current_hash);
        while let Some(hash) = cursor {
            if !chain_hashes.insert(hash) {
                bail!("cycle detected in state-diff ancestry at hash {hash}");
            }
            let value = db
                .get_cf(&cf, hash.to_be_bytes())
                .with_context(|| format!("failed to read state diff {hash}"))?
                .with_context(|| format!("state diff {hash} is missing"))?;
            let diff =
                parse_state_diff(&value).with_context(|| format!("invalid state diff {hash}"))?;
            cursor = diff.parent;
            chain.push(diff);
        }
        chain.reverse();
        event_ids.extend(apply_state_diff_chain(&chain).iter().map(short_event_id));
    }

    Ok(event_ids)
}

fn load_retained_short_event_ids(db: &RocksDb) -> Result<(HashSet<u64>, u64)> {
    let cf = required_cf(db, "shortstatehash_statediff")?;
    let mut event_ids = HashSet::new();
    let mut state_hashes = 0_u64;

    for item in db.iterator_cf(&cf, IteratorMode::Start) {
        let (key, value) = item.context("failed while scanning retained state diffs")?;
        let hash = key
            .as_ref()
            .try_into()
            .map(u64::from_be_bytes)
            .with_context(|| {
                format!(
                    "retained state-diff key is not eight bytes: {}",
                    hex::encode(&key)
                )
            })?;
        let diff = parse_state_diff(&value)
            .with_context(|| format!("invalid retained state diff {hash}"))?;
        event_ids.extend(diff.added.iter().chain(&diff.removed).map(short_event_id));
        state_hashes = state_hashes.saturating_add(1);
    }

    Ok((event_ids, state_hashes))
}

fn load_state_event_ids(db: &RocksDb, short_event_ids: &HashSet<u64>) -> Result<HashSet<String>> {
    let cf = required_cf(db, "shorteventid_eventid")?;
    short_event_ids
        .iter()
        .map(|short_id| {
            let value = db
                .get_cf(&cf, short_id.to_be_bytes())
                .with_context(|| format!("failed to resolve short event id {short_id}"))?
                .with_context(|| format!("short event id {short_id} has no event id"))?;
            let event_id = std::str::from_utf8(&value)
                .with_context(|| format!("event id for short id {short_id} is not UTF-8"))?;
            if !event_id.starts_with('$') {
                bail!("event id for short id {short_id} is malformed");
            }
            Ok(event_id.to_owned())
        })
        .collect()
}

fn scan_timeline_state_events(db: &RocksDb) -> Result<(HashMap<String, StateEventRecord>, u64)> {
    let cf = required_cf(db, "pduid_pdu")?;
    let mut state_events = HashMap::new();
    let mut scanned = 0_u64;

    for item in db.iterator_cf(&cf, IteratorMode::Start) {
        let (key, value) = item.context("failed while scanning timeline PDUs")?;
        scanned = scanned.saturating_add(1);
        let Some(metadata) = parse_state_event_metadata(&value, None)
            .with_context(|| format!("invalid timeline PDU at key {}", hex::encode(&key)))?
        else {
            continue;
        };
        let room_id = metadata
            .room_id
            .as_deref()
            .context("timeline state event has no room_id")?;
        let count = pdu_count_bytes(&key)?;
        let index_key = timeline_index_key(room_id, metadata.origin_server_ts, count);
        let event_id = metadata.event_id.clone();
        let previous = state_events.insert(
            event_id.clone(),
            StateEventRecord {
                metadata,
                timeline: Some(TimelineLocation {
                    pdu_key: key,
                    index_key,
                    payload_bytes: value.len() as u64,
                }),
                outlier_payload_bytes: 0,
            },
        );
        if previous.is_some() {
            bail!("duplicate timeline state event id: {event_id}");
        }
    }

    Ok((state_events, scanned))
}

fn scan_outlier_state_events(
    db: &RocksDb,
    state_events: &mut HashMap<String, StateEventRecord>,
) -> Result<u64> {
    let cf = required_cf(db, "eventid_outlierpdu")?;
    let mut scanned = 0_u64;

    for item in db.iterator_cf(&cf, IteratorMode::Start) {
        let (key, value) = item.context("failed while scanning outlier PDUs")?;
        scanned = scanned.saturating_add(1);
        let key_event_id = std::str::from_utf8(&key).context("outlier event id is not UTF-8")?;
        let Some(metadata) = parse_state_event_metadata(&value, Some(key_event_id))
            .with_context(|| format!("invalid outlier PDU for {key_event_id}"))?
        else {
            continue;
        };
        if metadata.event_id != key_event_id {
            bail!("outlier key and payload event id disagree for {key_event_id}");
        }

        match state_events.get_mut(key_event_id) {
            Some(existing) => {
                if existing.metadata.origin_server_ts != metadata.origin_server_ts
                    || existing.metadata.auth_events != metadata.auth_events
                {
                    bail!("timeline and outlier payloads disagree for {key_event_id}");
                }
                existing.outlier_payload_bytes = value.len() as u64;
            }
            None => {
                state_events.insert(
                    key_event_id.to_owned(),
                    StateEventRecord {
                        metadata,
                        timeline: None,
                        outlier_payload_bytes: value.len() as u64,
                    },
                );
            }
        }
    }

    Ok(scanned)
}

fn metadata_map(
    records: &HashMap<String, StateEventRecord>,
) -> HashMap<String, StateEventMetadata> {
    records
        .iter()
        .map(|(event_id, record)| (event_id.clone(), record.metadata.clone()))
        .collect()
}

#[derive(Default)]
struct DeletionPlan {
    timeline_payloads_deleted: u64,
    outlier_payloads_deleted: u64,
    original_payload_keys_deleted: u64,
    estimated_payload_bytes_deleted: u64,
}

fn verify_deletion_plan(
    db: &RocksDb,
    records: &HashMap<String, StateEventRecord>,
    candidates: &[String],
) -> Result<DeletionPlan> {
    let eventid_pduid = required_cf(db, "eventid_pduid")?;
    let timeline_index = required_cf(db, "roomid_tscount_pducount")?;
    let outliers = required_cf(db, "eventid_outlierpdu")?;
    let originals = required_cf(db, "eventid_originalpdu")?;
    let mut plan = DeletionPlan::default();

    for event_id in candidates {
        let record = records
            .get(event_id)
            .with_context(|| format!("candidate event disappeared from audit map: {event_id}"))?;
        if let Some(timeline) = &record.timeline {
            let mapped_key = db
                .get_cf(&eventid_pduid, event_id.as_bytes())
                .with_context(|| format!("failed to verify timeline mapping for {event_id}"))?
                .with_context(|| format!("timeline mapping is missing for {event_id}"))?;
            if mapped_key.as_slice() != timeline.pdu_key.as_ref() {
                bail!("timeline mapping points at a different PDU for {event_id}");
            }
            let count = pdu_count_bytes(&timeline.pdu_key)?;
            let indexed_count = db
                .get_cf(&timeline_index, &timeline.index_key)
                .with_context(|| format!("failed to verify timeline index for {event_id}"))?
                .with_context(|| format!("timeline index is missing for {event_id}"))?;
            if indexed_count.as_slice() != count {
                bail!("timeline index points at a different PDU for {event_id}");
            }
            plan.timeline_payloads_deleted = plan.timeline_payloads_deleted.saturating_add(1);
            plan.estimated_payload_bytes_deleted = plan
                .estimated_payload_bytes_deleted
                .saturating_add(timeline.payload_bytes);
        }
        if record.outlier_payload_bytes > 0 {
            if db
                .get_cf(&outliers, event_id.as_bytes())
                .with_context(|| format!("failed to verify outlier payload for {event_id}"))?
                .is_none()
            {
                bail!("outlier payload is missing for {event_id}");
            }
            plan.outlier_payloads_deleted = plan.outlier_payloads_deleted.saturating_add(1);
            plan.estimated_payload_bytes_deleted = plan
                .estimated_payload_bytes_deleted
                .saturating_add(record.outlier_payload_bytes);
        }
        if let Some(original) = db
            .get_cf(&originals, event_id.as_bytes())
            .with_context(|| format!("failed to inspect retained original for {event_id}"))?
        {
            plan.original_payload_keys_deleted =
                plan.original_payload_keys_deleted.saturating_add(1);
            plan.estimated_payload_bytes_deleted = plan
                .estimated_payload_bytes_deleted
                .saturating_add(original.len() as u64);
        }
    }

    Ok(plan)
}

fn apply_deletion_plan(
    db: &RocksDb,
    batch: &mut WriteBatch,
    records: &HashMap<String, StateEventRecord>,
    candidates: &[String],
) -> Result<()> {
    let pduid_pdu = required_cf(db, "pduid_pdu")?;
    let eventid_pduid = required_cf(db, "eventid_pduid")?;
    let outliers = required_cf(db, "eventid_outlierpdu")?;
    let originals = required_cf(db, "eventid_originalpdu")?;
    let timeline_index = required_cf(db, "roomid_tscount_pducount")?;

    for event_id in candidates {
        let record = records
            .get(event_id)
            .with_context(|| format!("candidate event disappeared from audit map: {event_id}"))?;
        if let Some(timeline) = &record.timeline {
            batch.delete_cf(&pduid_pdu, &timeline.pdu_key);
            batch.delete_cf(&eventid_pduid, event_id.as_bytes());
            batch.delete_cf(&timeline_index, &timeline.index_key);
        }
        if record.outlier_payload_bytes > 0 {
            batch.delete_cf(&outliers, event_id.as_bytes());
        }
        batch.delete_cf(&originals, event_id.as_bytes());
    }
    Ok(())
}

fn required_cf<'a>(
    db: &'a RocksDb,
    name: &str,
) -> Result<std::sync::Arc<rocksdb::BoundColumnFamily<'a>>> {
    db.cf_handle(name)
        .with_context(|| format!("RocksDB column family disappeared: {name}"))
}

fn env_bool(name: &str, default: bool) -> bool {
    env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn now_epoch_secs() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time is before Unix epoch")?
        .as_secs())
}

fn directory_size(path: &Path) -> u64 {
    fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| {
            entry
                .metadata()
                .map(|metadata| {
                    if metadata.is_dir() {
                        directory_size(&entry.path())
                    } else {
                        metadata.len()
                    }
                })
                .unwrap_or(0)
        })
        .sum()
}

fn filesystem_available_bytes(path: &Path) -> Result<u64> {
    let output = Command::new("df")
        .arg("-Pk")
        .arg(path)
        .output()
        .context("failed to execute df for compaction headroom")?;
    if !output.status.success() {
        bail!("df failed while checking compaction headroom");
    }
    let stdout = String::from_utf8(output.stdout).context("df output is not UTF-8")?;
    let available_kib = stdout
        .lines()
        .nth(1)
        .and_then(|line| line.split_whitespace().nth(3))
        .context("df output did not contain available KiB")?
        .parse::<u64>()
        .context("df available KiB is not an integer")?;
    Ok(available_kib.saturating_mul(1024))
}

fn write_json_status<T: Serialize>(path: &Path, status: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create status directory {}", parent.display()))?;
    }
    let temporary = PathBuf::from(format!("{}.tmp", path.display()));
    fs::write(&temporary, serde_json::to_vec_pretty(status)?)
        .with_context(|| format!("failed to write {}", temporary.display()))?;
    fs::rename(&temporary, path)
        .with_context(|| format!("failed to publish {}", path.display()))?;
    Ok(())
}
