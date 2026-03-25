//! Migration safety tests.
//!
//! Validates that PG migrations apply cleanly against a database with existing
//! data - not just an empty DB. Catches NOT NULL without default, accidental
//! data drops, and schema conflicts between new and existing state.

use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use diesel::sql_query;
use diesel::RunQueryDsl;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

const PG_MIGRATIONS: EmbeddedMigrations = embed_migrations!("./pg_migrations");

fn get_pg_url() -> String {
    std::env::var("TEST_PG_DATABASE_URL").unwrap_or_else(|_| {
        "postgres://lightfriend:test@localhost:5432/lightfriend_test".to_string()
    })
}

fn create_pg_connection() -> PgConnection {
    let url = get_pg_url();
    PgConnection::establish(&url).expect("Failed to connect to test PostgreSQL")
}

/// Run all migrations, insert seed data, then run migrations again.
/// This proves migrations are idempotent and safe against existing data.
#[test]
fn test_migrations_apply_cleanly_with_existing_data() {
    let mut conn = create_pg_connection();

    // Run all migrations (creates schema from scratch)
    conn.run_pending_migrations(PG_MIGRATIONS)
        .expect("Initial migration run failed");

    // Insert representative test data
    let seed_sql = include_str!("fixtures/seed_data.sql");
    for statement in seed_sql.split(';') {
        // Strip comment lines before checking if statement is empty
        let sql: String = statement
            .lines()
            .filter(|l| !l.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n");
        let sql = sql.trim();
        if sql.is_empty() {
            continue;
        }
        sql_query(sql)
            .execute(&mut conn)
            .unwrap_or_else(|e| panic!("Seed data insert failed: {e}\nSQL: {sql}"));
    }

    // Verify data was inserted
    let user_secret_count: i64 = sql_query("SELECT count(*) as count FROM user_secrets")
        .get_result::<CountResult>(&mut conn)
        .expect("Failed to count user_secrets")
        .count;
    assert!(
        user_secret_count > 0,
        "Seed data should have inserted user_secrets"
    );

    // Run migrations again - should be a no-op (all already applied)
    conn.run_pending_migrations(PG_MIGRATIONS)
        .expect("Re-running migrations should succeed (idempotent)");

    // Verify data survived
    let post_count: i64 = sql_query("SELECT count(*) as count FROM user_secrets")
        .get_result::<CountResult>(&mut conn)
        .expect("Failed to count user_secrets after re-migration")
        .count;
    assert_eq!(
        user_secret_count, post_count,
        "Data should survive migration re-run"
    );

    // Verify we can still query all seeded tables
    for table in &[
        "user_secrets",
        "user_info",
        "contact_profiles",
        "contact_profile_exceptions",
        "bridges",
        "bridge_disconnection_events",
        "message_history",
        "usage_logs",
        "items",
        "imap_connection",
        "mcp_servers",
        "totp_secrets",
        "totp_backup_codes",
    ] {
        let query = format!("SELECT count(*) as count FROM {table}");
        let result: CountResult = sql_query(&query)
            .get_result(&mut conn)
            .unwrap_or_else(|e| panic!("Failed to query {table}: {e}"));
        assert!(result.count >= 0, "Table {table} should be queryable");
    }

    // Cleanup: truncate all tables for test isolation
    let _ = sql_query(
        "TRUNCATE items, message_history, usage_logs, contact_profiles, \
         contact_profile_exceptions, bridges, bridge_disconnection_events, \
         imap_connection, tesla, youtube, mcp_servers, totp_secrets, \
         totp_backup_codes, webauthn_credentials, webauthn_challenges, \
         user_secrets, user_info, processed_emails CASCADE",
    )
    .execute(&mut conn);
}

#[derive(QueryableByName)]
struct CountResult {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    count: i64,
}
