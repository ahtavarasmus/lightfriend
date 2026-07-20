use axum::{routing::get, Router};
use backend::handlers::health_handlers::{parse_storage_health_json, storage_health};

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
