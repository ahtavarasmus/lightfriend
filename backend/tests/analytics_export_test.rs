use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    middleware,
    routing::get,
    Router,
};
use backend::{
    handlers::{admin_export_handlers::export_analytics, auth_middleware::require_admin},
    repositories::analytics_export_repository::write_archive,
    test_utils::create_test_state,
};
use diesel::{connection::SimpleConnection, prelude::*};
use serde_json::{json, Value};
use serial_test::serial;
use tower::ServiceExt;

fn cookie(user_id: i32) -> String {
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &json!({"sub": user_id, "exp": chrono::Utc::now().timestamp() + 3600}),
        &jsonwebtoken::EncodingKey::from_secret(b"analytics-test-secret"),
    )
    .unwrap();
    format!("access_token={token}")
}

#[tokio::test]
#[serial]
async fn analytics_export_preserves_history_excludes_content_and_enforces_admin_access() {
    struct RestoreEnv(Vec<(&'static str, Option<String>)>);
    impl Drop for RestoreEnv {
        fn drop(&mut self) {
            for (key, value) in &self.0 {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
    let _restore = RestoreEnv(
        ["JWT_SECRET_KEY", "ADMIN_EMAILS"]
            .into_iter()
            .map(|key| (key, std::env::var(key).ok()))
            .collect(),
    );
    std::env::set_var("JWT_SECRET_KEY", "analytics-test-secret");
    std::env::set_var("ADMIN_EMAILS", "export-admin@example.test");
    let state = create_test_state();
    let mut conn = state.pg_pool.get().expect("This test requires PostgreSQL");
    conn.batch_execute(
        r#"
        TRUNCATE llm_usage_logs, bridge_bandwidth_logs, ai_model_prices;
        INSERT INTO users (id, email, password_hash, phone_number, nickname, credits, credits_left,
            charge_when_under, stripe_customer_id, stripe_payment_method_id, magic_token)
        VALUES (910001, 'export-admin@example.test', 'DO_NOT_EXPORT_password', '+358409876543',
            'DO_NOT_EXPORT_name', 1.25, 2.5, false, 'cus_analytics_test',
            'DO_NOT_EXPORT_payment_method', 'DO_NOT_EXPORT_token'),
            (910002, '=formula,"quoted"@example.test', 'DO_NOT_EXPORT_password', '+358409876544',
            NULL, 0, 0, false, NULL, NULL, NULL);
        INSERT INTO llm_usage_logs (user_id, provider, model, callsite, prompt_tokens,
            completion_tokens, total_tokens, created_at)
        SELECT 919999, 'test', 'model', 'chat', n, 10, n + 10, n FROM generate_series(1, 1005) n;
        INSERT INTO llm_usage_logs (user_id, provider, model, callsite, prompt_tokens,
            completion_tokens, total_tokens, cached_prompt_tokens, provider_cost_usd,
            customer_cost_usd, pricing_snapshot, created_at)
        VALUES (910001, 'test', 'model', 'chat', 40, 10, 50, 20, 0.0000123456789,
            0.0000246913578, 'DO_NOT_EXPORT_invalid_pricing', 1600000000);
        INSERT INTO message_status_log (message_sid, user_id, direction, to_number, from_number,
            status, error_message, encrypted_body, created_at, updated_at, price, price_unit,
            fallback_error)
        VALUES ('SM_analytics_test', 910001, 'outbound', '+358409876543', '+358409876544',
            'delivered', 'DO_NOT_EXPORT_sms_error', 'DO_NOT_EXPORT_sms_body',
            1600000000, 1600000001, -0.0125, 'EUR', 'DO_NOT_EXPORT_fallback_error');
        INSERT INTO usage_logs (user_id, sid, activity_type, credits, created_at, time_consumed,
            success, reason, status, call_duration)
        VALUES (910001, 'CA_analytics_test', 'call', 0.25, 1600000000, 3, true,
            'DO_NOT_EXPORT_reason', 'completed', 62);
        INSERT INTO usage_logs (user_id, activity_type, created_at, credits)
        VALUES (910001, 'DO_NOT_EXPORT_admin_email_subject', 1600000001, 0.05);
        INSERT INTO billing_accounts (user_id, metronome_customer_id, provisioning_error, created_at, updated_at)
        VALUES (910001, 'metronome_analytics_test', 'DO_NOT_EXPORT_provisioning', 1600000000, 1600000000);
        INSERT INTO billing_usage_events (transaction_id, user_id, event_type, cost_microusd,
            occurred_at, status, next_attempt_at, last_error, created_at)
        VALUES ('analytics-test-transaction', 910001, 'voice', 123456789, 1600000000,
            'failed', 1600000100, 'DO_NOT_EXPORT_billing_error', 1600000001);
        INSERT INTO processed_emails (user_id, email_uid, processed_at)
        VALUES (910001, 'DO_NOT_EXPORT_email_uid_1', 1600000000),
            (910001, 'DO_NOT_EXPORT_email_uid_2', 1600000000);
        INSERT INTO light_tool_devices (id, installation_id_hash, device_token_hash,
            user_id, trial_started_at, trial_expires_at, last_seen_at, created_at, updated_at)
        VALUES (910001, 'DO_NOT_EXPORT_installation', 'DO_NOT_EXPORT_device_token',
            910001, 1600000000, 1600000001, 1600000000, 1600000000, 1600000000);
        INSERT INTO light_tool_runs (id, device_id, account_user_id, client_message_id,
            encrypted_user_message, encrypted_image_data_url, encrypted_activity_text,
            encrypted_assistant_message, encrypted_error_message, status, created_at, updated_at)
        VALUES ('analytics-run', 910001, 910001, 'DO_NOT_EXPORT_client_message',
            'DO_NOT_EXPORT_prompt', 'DO_NOT_EXPORT_image', 'DO_NOT_EXPORT_activity',
            'DO_NOT_EXPORT_response', 'DO_NOT_EXPORT_run_error', 'completed', 1600000000, 1600000001);
        INSERT INTO waitlist (email, created_at) VALUES ('waitlist@example.test', 1500000000);
        "#,
    ).unwrap();
    let snapshot = json!({
        "quote": {"rates": {"input_per_million": 1.5, "output_per_million": 3.0,
            "cached_input_per_million": 0.25, "request_usd": 0.0, "search_usd": 0.0,
            "extra": "DO_NOT_EXPORT_nested"}, "source": "default_model", "fetched_at": 1590000000,
            "extra": "DO_NOT_EXPORT_quote"},
        "markup": 2.0, "provider_cost_usd": 0.1, "customer_cost_usd": 0.2,
        "reported_cost": false, "usage_complete": true, "message": "DO_NOT_EXPORT_snapshot"
    });
    diesel::sql_query("INSERT INTO llm_usage_logs (user_id, provider, model, callsite, created_at, pricing_snapshot) VALUES (910001, 'test', 'model', 'chat', 1600000001, $1)")
        .bind::<diesel::sql_types::Text, _>(snapshot.to_string()).execute(&mut conn).unwrap();
    diesel::sql_query("INSERT INTO ai_model_prices (provider, model, rates, fetched_at) VALUES ('analytics-test', 'model', $1, 1600000000) ON CONFLICT (provider, model) DO UPDATE SET rates = EXCLUDED.rates")
        .bind::<diesel::sql_types::Text, _>(snapshot["quote"]["rates"].to_string()).execute(&mut conn).unwrap();
    drop(conn);

    let app = Router::new()
        .route("/api/admin/exports/analytics", get(export_analytics))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_admin))
        .with_state(state.clone());
    for (auth_cookie, expected) in [
        (None, StatusCode::UNAUTHORIZED),
        (Some(cookie(910002)), StatusCode::FORBIDDEN),
    ] {
        let mut request = Request::builder().uri("/api/admin/exports/analytics");
        if let Some(cookie) = auth_cookie {
            request = request.header("cookie", cookie);
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/exports/analytics")
                .header("cookie", cookie(910001))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store, private");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert!(response.headers()["content-disposition"]
        .to_str()
        .unwrap()
        .ends_with(".json\""));
    let declared_length: usize = response.headers()["content-length"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(bytes.len(), declared_length);
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(!text.contains("DO_NOT_EXPORT"));
    assert!(!text.contains("+35840987654"));
    let archive: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(archive["schema_version"], 1);
    assert_eq!(archive["manifest"]["complete"], true);
    let datasets = &archive["datasets"];
    assert_eq!(datasets.as_object().unwrap().len(), 19);
    for summary in archive["manifest"]["datasets"].as_array().unwrap() {
        let rows = datasets[summary["name"].as_str().unwrap()]
            .as_array()
            .unwrap();
        assert_eq!(summary["row_count"].as_u64().unwrap(), rows.len() as u64);
        let times: Vec<i64> = summary["timestamp_field"]
            .as_str()
            .map(|field| rows.iter().filter_map(|row| row[field].as_i64()).collect())
            .unwrap_or_default();
        assert_eq!(summary["earliest_timestamp"], json!(times.iter().min()));
        assert_eq!(summary["latest_timestamp"], json!(times.iter().max()));
    }
    assert_eq!(
        datasets["users"][1]["email"],
        "=formula,\"quoted\"@example.test"
    );
    assert_eq!(
        datasets["users"][0]["stripe_customer_id"],
        "cus_analytics_test"
    );
    let llm = datasets["llm_usage"].as_array().unwrap();
    let historical: Vec<_> = llm.iter().filter(|row| row["user_id"] == 919999).collect();
    assert_eq!(historical.len(), 1005);
    assert_eq!(historical[0]["created_at"], 1);
    assert_eq!(historical[1004]["created_at"], 1005);
    assert!(historical[0]["provider_cost_usd"].is_null());
    assert!(historical[0]["cached_prompt_tokens"].is_null());
    let priced = llm
        .iter()
        .find(|row| row["user_id"] == 910001 && row["created_at"] == 1600000000)
        .unwrap();
    assert_eq!(priced["provider_cost_usd"], json!(0.0000123456789));
    assert!(priced["pricing_snapshot"].is_null());
    assert_eq!(priced["snapshot_export_status"], "invalid");
    let snapshot_row = llm
        .iter()
        .find(|row| row["user_id"] == 910001 && row["created_at"] == 1600000001)
        .unwrap();
    assert_eq!(
        snapshot_row["pricing_snapshot"]["quote"]["source"],
        "default_model"
    );
    assert_eq!(
        snapshot_row["pricing_snapshot"]["quote"]["rates"]["input_per_million"],
        1.5
    );
    assert_eq!(datasets["sms_usage"][0]["price_unit"], "EUR");
    assert!(datasets["sms_usage"][0]["price"].as_f64().unwrap() < 0.0);
    assert_eq!(datasets["legacy_usage"][0]["call_duration"], 62);
    assert_eq!(datasets["legacy_usage"][0]["activity_type_redacted"], false);
    assert_eq!(
        datasets["legacy_usage"][1]["activity_type"],
        "other_redacted"
    );
    assert_eq!(datasets["legacy_usage"][1]["activity_type_redacted"], true);
    assert_eq!(datasets["billing_usage"][0]["cost_microusd"], 123456789);
    assert_eq!(datasets["billing_usage"][0]["status"], "failed");
    assert_eq!(datasets["processed_email_counts"][0]["processed_count"], 2);
    assert_eq!(datasets["light_tool_runs"][0]["account_user_id"], 910001);
    // Empty tables remain explicit datasets, not skipped queries.
    assert_eq!(datasets["billing_usage_intents"], json!([]));

    // A concurrent write after the first dataset must not enter later datasets.
    struct ConcurrentWriter {
        bytes: Vec<u8>,
        other: diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<PgConnection>>,
        inserted: bool,
    }
    impl std::io::Write for ConcurrentWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.bytes.extend_from_slice(bytes);
            if !self.inserted && self.bytes.windows(9).any(|w| w == b"llm_usage") {
                self.other.batch_execute("INSERT INTO llm_usage_logs (user_id, provider, model, callsite, created_at) VALUES (918888, 'test', 'concurrent', 'chat', 1700000000)").unwrap();
                self.inserted = true;
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut conn = state.pg_pool.get().unwrap();
    let mut writer = ConcurrentWriter {
        bytes: Vec::new(),
        other: state.pg_pool.get().unwrap(),
        inserted: false,
    };
    write_archive(&mut conn, &mut writer).unwrap();
    assert!(writer.inserted);
    let consistent: Value = serde_json::from_slice(&writer.bytes).unwrap();
    assert!(!consistent["datasets"]["llm_usage"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["user_id"] == 918888));
    drop(writer);
    let mut next = Vec::new();
    write_archive(&mut conn, &mut next).unwrap();
    let next: Value = serde_json::from_slice(&next).unwrap();
    assert!(next["datasets"]["llm_usage"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["user_id"] == 918888));

    // Failed output returns an error and rolls back the read-only transaction.
    let mut too_small = [0u8; 10];
    assert!(write_archive(&mut conn, &mut &mut too_small[..]).is_err());
    let mut after_failure = Vec::new();
    write_archive(&mut conn, &mut after_failure).unwrap();
    assert!(
        serde_json::from_slice::<Value>(&after_failure).unwrap()["manifest"]["complete"]
            .as_bool()
            .unwrap()
    );
    // Missing tables must fail, never silently turn into empty datasets.
    conn.batch_execute("SET search_path TO pg_catalog").unwrap();
    let mut failed = Vec::new();
    assert!(write_archive(&mut conn, &mut failed).is_err());
    assert!(serde_json::from_slice::<Value>(&failed).is_err());
    conn.batch_execute("SET search_path TO public").unwrap();
}
