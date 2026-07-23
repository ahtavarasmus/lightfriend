use axum::{routing::get, Router};
use backend::handlers::health_handlers::{
    parse_hourly_backup_status_json, parse_storage_health_json, storage_health,
};
use chrono::{TimeZone, Utc};

#[test]
fn storage_health_is_a_valid_axum_get_handler() {
    let _: Router = Router::new().route("/api/internal/health/storage", get(storage_health));
}

#[test]
fn storage_health_json_accepts_aggregate_metrics() {
    let input = br#"{
        "timestamp":"2026-07-20T17:33:58Z",
        "filesystems":{
            "root":{"size_kib":1000,"used_kib":750,"avail_kib":250,"use_pct":75},
            "tmp":{"size_kib":2000,"used_kib":100,"avail_kib":1900,"use_pct":5}
        },
        "reserve":{"path":"/internal/path","present":true,"bytes":314572800,"projected_root_avail_kib":557200},
        "tuwunel":{
            "total_bytes":275139319,
            "media":{"count":0,"bytes":0},
            "rocksdb_sst":{"count":447,"bytes":272313766},
            "rocksdb_archive_log":{"count":13,"bytes":625892},
            "rocksdb_meta_logs":{"count":3,"bytes":1517127},
            "other":{"count":2,"bytes":52}
        },
        "postgres":{"bytes":351747689},
        "tuwunel_backup_engine":{"bytes":0},
        "supervisor_logs":{"bytes":4718592}
    }"#;

    let metrics = parse_storage_health_json(input).expect("valid storage metrics");

    assert_eq!(metrics.filesystems.root.use_pct, 75);
    assert!(metrics.reserve.present);
    assert_eq!(metrics.tuwunel.total_bytes, 275_139_319_u64);

    let response = serde_json::to_value(metrics).expect("serialize storage metrics");
    assert!(response["reserve"].get("path").is_none());
}

#[test]
fn storage_health_json_rejects_malformed_output() {
    assert!(parse_storage_health_json(b"not json").is_err());
}

#[test]
fn hourly_backup_health_reports_a_fresh_success() {
    let now = Utc
        .with_ymd_and_hms(2026, 7, 23, 18, 30, 0)
        .single()
        .expect("valid current time");
    let health = parse_hourly_backup_status_json(
        br#"{
            "status":"SUCCESS",
            "timestamp":"20260723T180000Z",
            "file":"lightfriend-full-backup-secret-name.tar.gz.enc",
            "size_bytes":123
        }"#,
        now,
    );

    let response = serde_json::to_value(health).expect("serialize backup health");
    assert_eq!(response["status"], "success");
    assert_eq!(response["last_attempt_at"], "2026-07-23T18:00:00Z");
    assert_eq!(response["age_seconds"], 1800);
    assert_eq!(response["stale"], false);
    assert!(response.get("file").is_none());
    assert!(response.get("size_bytes").is_none());
}

#[test]
fn hourly_backup_health_reports_a_sanitized_failure() {
    let now = Utc
        .with_ymd_and_hms(2026, 7, 23, 18, 10, 0)
        .single()
        .expect("valid current time");
    let health = parse_hourly_backup_status_json(
        br#"{
            "status":"FAILED",
            "timestamp":"20260723T180500Z",
            "error":"raw internal diagnostic that must not be exposed",
            "step":"upload-s3",
            "path":"/internal/path"
        }"#,
        now,
    );

    let response = serde_json::to_value(health).expect("serialize backup health");
    assert_eq!(response["status"], "failed");
    assert_eq!(response["last_attempt_at"], "2026-07-23T18:05:00Z");
    assert_eq!(response["age_seconds"], 300);
    assert_eq!(response["stale"], false);
    assert_eq!(response["failed_step"], "upload-s3");
    assert!(response.get("error").is_none());
    assert!(response.get("path").is_none());
}

#[test]
fn hourly_backup_health_marks_old_or_unreadable_status_as_stale() {
    let now = Utc
        .with_ymd_and_hms(2026, 7, 23, 20, 0, 1)
        .single()
        .expect("valid current time");
    let old = parse_hourly_backup_status_json(
        br#"{"status":"SUCCESS","timestamp":"20260723T180000Z"}"#,
        now,
    );
    let unreadable = parse_hourly_backup_status_json(b"not json", now);

    assert!(old.stale);
    assert_eq!(old.age_seconds, Some(7201));
    assert_eq!(unreadable.status, "unknown");
    assert!(unreadable.stale);
    assert_eq!(unreadable.age_seconds, None);
}

#[test]
fn hourly_backup_health_drops_unsafe_failure_steps() {
    let now = Utc
        .with_ymd_and_hms(2026, 7, 23, 18, 10, 0)
        .single()
        .expect("valid current time");
    let health = parse_hourly_backup_status_json(
        br#"{
            "status":"FAILED",
            "timestamp":"20260723T180500Z",
            "step":"upload failed at /internal/path"
        }"#,
        now,
    );

    assert_eq!(health.status, "failed");
    assert_eq!(health.failed_step, None);
}
