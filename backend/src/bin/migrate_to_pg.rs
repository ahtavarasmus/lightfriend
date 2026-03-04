//! Data migration binary: copies sensitive data from SQLite to PostgreSQL.
//!
//! Reads from SQLite (source of truth before migration), writes to PG.
//! Idempotent: uses ON CONFLICT DO NOTHING so safe to run multiple times.
//!
//! Must run AFTER PG tables exist (PG migrations applied) but BEFORE
//! SQLite cleanup drops the old tables/columns.
//!
//! Usage: cargo run --bin migrate_to_pg

use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Float, Integer, Nullable, Text};
use std::env;

fn main() {
    dotenvy::dotenv().ok();

    let sqlite_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pg_url = env::var("PG_DATABASE_URL").expect("PG_DATABASE_URL must be set");

    let mut sqlite_conn =
        SqliteConnection::establish(&sqlite_url).expect("Failed to connect to SQLite");
    let mut pg_conn = PgConnection::establish(&pg_url).expect("Failed to connect to PostgreSQL");

    println!("Connected to both databases. Starting migration...");

    // 1. Migrate user_secrets (extracted from users + user_settings)
    migrate_user_secrets(&mut sqlite_conn, &mut pg_conn);

    // 2. Migrate whole tables
    migrate_user_info(&mut sqlite_conn, &mut pg_conn);
    migrate_contact_profiles(&mut sqlite_conn, &mut pg_conn);
    migrate_contact_profile_exceptions(&mut sqlite_conn, &mut pg_conn);
    migrate_imap_connection(&mut sqlite_conn, &mut pg_conn);
    migrate_message_history(&mut sqlite_conn, &mut pg_conn);
    migrate_tesla(&mut sqlite_conn, &mut pg_conn);
    migrate_youtube(&mut sqlite_conn, &mut pg_conn);
    migrate_mcp_servers(&mut sqlite_conn, &mut pg_conn);
    migrate_totp_secrets(&mut sqlite_conn, &mut pg_conn);
    migrate_totp_backup_codes(&mut sqlite_conn, &mut pg_conn);
    migrate_webauthn_credentials(&mut sqlite_conn, &mut pg_conn);
    migrate_webauthn_challenges(&mut sqlite_conn, &mut pg_conn);
    migrate_items(&mut sqlite_conn, &mut pg_conn);
    migrate_bridges(&mut sqlite_conn, &mut pg_conn);
    migrate_bridge_disconnection_events(&mut sqlite_conn, &mut pg_conn);
    migrate_usage_logs(&mut sqlite_conn, &mut pg_conn);
    migrate_processed_emails(&mut sqlite_conn, &mut pg_conn);

    // Reset all PG sequences to max(id) so new inserts get correct IDs
    reset_sequences(&mut pg_conn);

    println!("Migration complete!");
}

// Helper macro: read from SQLite with raw SQL, insert into PG with raw SQL.
// This avoids needing both SQLite and PG schema modules to compile against both backends.

fn migrate_user_secrets(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    // Extract secrets from users table
    #[derive(QueryableByName, Debug)]
    struct UserSecret {
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Nullable<Text>)]
        matrix_username: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        matrix_device_id: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        encrypted_matrix_access_token: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        encrypted_matrix_password: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        encrypted_matrix_secret_storage_recovery_key: Option<String>,
    }

    #[derive(QueryableByName, Debug)]
    struct UserTwilio {
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Nullable<Text>)]
        encrypted_twilio_account_sid: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        encrypted_twilio_auth_token: Option<String>,
    }

    let users: Vec<UserSecret> = sql_query(
        "SELECT id as user_id, matrix_username, matrix_device_id, \
         encrypted_matrix_access_token, encrypted_matrix_password, \
         encrypted_matrix_secret_storage_recovery_key FROM users",
    )
    .load(sqlite)
    .expect("Failed to read users");

    let settings: Vec<UserTwilio> = sql_query(
        "SELECT user_id, encrypted_twilio_account_sid, encrypted_twilio_auth_token FROM user_settings",
    )
    .load(sqlite)
    .expect("Failed to read user_settings");

    // Build a map of user_id -> twilio creds
    let twilio_map: std::collections::HashMap<i32, (Option<String>, Option<String>)> = settings
        .into_iter()
        .map(|s| {
            (
                s.user_id,
                (
                    s.encrypted_twilio_account_sid,
                    s.encrypted_twilio_auth_token,
                ),
            )
        })
        .collect();

    let mut count = 0;
    for u in &users {
        let (twilio_sid, twilio_token) =
            twilio_map.get(&u.user_id).cloned().unwrap_or((None, None));

        let result = diesel::sql_query(
            "INSERT INTO user_secrets (user_id, matrix_username, matrix_device_id, \
             encrypted_matrix_access_token, encrypted_matrix_password, \
             encrypted_matrix_secret_storage_recovery_key, \
             encrypted_twilio_account_sid, encrypted_twilio_auth_token) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
             ON CONFLICT (user_id) DO NOTHING",
        )
        .bind::<Integer, _>(u.user_id)
        .bind::<Nullable<Text>, _>(&u.matrix_username)
        .bind::<Nullable<Text>, _>(&u.matrix_device_id)
        .bind::<Nullable<Text>, _>(&u.encrypted_matrix_access_token)
        .bind::<Nullable<Text>, _>(&u.encrypted_matrix_password)
        .bind::<Nullable<Text>, _>(&u.encrypted_matrix_secret_storage_recovery_key)
        .bind::<Nullable<Text>, _>(&twilio_sid)
        .bind::<Nullable<Text>, _>(&twilio_token)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!(
                "  Error inserting user_secrets for user {}: {}",
                u.user_id, e
            ),
        }
    }
    println!("user_secrets: migrated {} rows", count);
}

fn migrate_user_info(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Nullable<Text>)]
        location: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        info: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        timezone: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        nearby_places: Option<String>,
        #[diesel(sql_type = Nullable<Float>)]
        latitude: Option<f32>,
        #[diesel(sql_type = Nullable<Float>)]
        longitude: Option<f32>,
    }

    let rows: Vec<Row> =
        sql_query("SELECT user_id, location, info, timezone, nearby_places, latitude, longitude FROM user_info")
            .load(sqlite)
            .expect("Failed to read user_info");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO user_info (user_id, location, info, timezone, nearby_places, latitude, longitude) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (user_id) DO NOTHING",
        )
        .bind::<Integer, _>(r.user_id)
        .bind::<Nullable<Text>, _>(&r.location)
        .bind::<Nullable<Text>, _>(&r.info)
        .bind::<Nullable<Text>, _>(&r.timezone)
        .bind::<Nullable<Text>, _>(&r.nearby_places)
        .bind::<Nullable<Float>, _>(r.latitude)
        .bind::<Nullable<Float>, _>(r.longitude)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error migrating user_info row: {}", e),
        }
    }
    println!("user_info: migrated {} rows", count);
}

fn migrate_contact_profiles(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Text)]
        nickname: String,
        #[diesel(sql_type = Nullable<Text>)]
        whatsapp_chat: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        telegram_chat: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        signal_chat: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        email_addresses: Option<String>,
        #[diesel(sql_type = Text)]
        notification_mode: String,
        #[diesel(sql_type = Text)]
        notification_type: String,
        #[diesel(sql_type = Integer)]
        notify_on_call: i32,
        #[diesel(sql_type = Integer)]
        created_at: i32,
        #[diesel(sql_type = Nullable<Text>)]
        whatsapp_room_id: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        telegram_room_id: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        signal_room_id: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        notes: Option<String>,
    }

    let rows: Vec<Row> = sql_query(
        "SELECT id, user_id, nickname, whatsapp_chat, telegram_chat, signal_chat, \
         email_addresses, notification_mode, notification_type, notify_on_call, \
         created_at, whatsapp_room_id, telegram_room_id, signal_room_id, notes \
         FROM contact_profiles",
    )
    .load(sqlite)
    .expect("Failed to read contact_profiles");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO contact_profiles (id, user_id, nickname, whatsapp_chat, telegram_chat, \
             signal_chat, email_addresses, notification_mode, notification_type, notify_on_call, \
             created_at, whatsapp_room_id, telegram_room_id, signal_room_id, notes) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Integer, _>(r.user_id)
        .bind::<Text, _>(&r.nickname)
        .bind::<Nullable<Text>, _>(&r.whatsapp_chat)
        .bind::<Nullable<Text>, _>(&r.telegram_chat)
        .bind::<Nullable<Text>, _>(&r.signal_chat)
        .bind::<Nullable<Text>, _>(&r.email_addresses)
        .bind::<Text, _>(&r.notification_mode)
        .bind::<Text, _>(&r.notification_type)
        .bind::<Integer, _>(r.notify_on_call)
        .bind::<Integer, _>(r.created_at)
        .bind::<Nullable<Text>, _>(&r.whatsapp_room_id)
        .bind::<Nullable<Text>, _>(&r.telegram_room_id)
        .bind::<Nullable<Text>, _>(&r.signal_room_id)
        .bind::<Nullable<Text>, _>(&r.notes)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error migrating contact_profiles row: {}", e),
        }
    }
    println!("contact_profiles: migrated {} rows", count);
}

fn migrate_contact_profile_exceptions(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Integer)]
        profile_id: i32,
        #[diesel(sql_type = Text)]
        platform: String,
        #[diesel(sql_type = Text)]
        notification_mode: String,
        #[diesel(sql_type = Text)]
        notification_type: String,
        #[diesel(sql_type = Integer)]
        notify_on_call: i32,
    }

    let rows: Vec<Row> = sql_query(
        "SELECT id, profile_id, platform, notification_mode, notification_type, notify_on_call \
         FROM contact_profile_exceptions",
    )
    .load(sqlite)
    .expect("Failed to read contact_profile_exceptions");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO contact_profile_exceptions (id, profile_id, platform, notification_mode, \
             notification_type, notify_on_call) VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Integer, _>(r.profile_id)
        .bind::<Text, _>(&r.platform)
        .bind::<Text, _>(&r.notification_mode)
        .bind::<Text, _>(&r.notification_type)
        .bind::<Integer, _>(r.notify_on_call)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error: {}", e),
        }
    }
    println!("contact_profile_exceptions: migrated {} rows", count);
}

fn migrate_imap_connection(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Text)]
        method: String,
        #[diesel(sql_type = Text)]
        encrypted_password: String,
        #[diesel(sql_type = Text)]
        status: String,
        #[diesel(sql_type = Integer)]
        last_update: i32,
        #[diesel(sql_type = Integer)]
        created_on: i32,
        #[diesel(sql_type = Text)]
        description: String,
        #[diesel(sql_type = Nullable<Text>)]
        imap_server: Option<String>,
        #[diesel(sql_type = Nullable<Integer>)]
        imap_port: Option<i32>,
    }

    let rows: Vec<Row> = sql_query(
        "SELECT id, user_id, method, encrypted_password, status, last_update, created_on, \
         description, imap_server, imap_port FROM imap_connection",
    )
    .load(sqlite)
    .expect("Failed to read imap_connection");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO imap_connection (id, user_id, method, encrypted_password, status, \
             last_update, created_on, description, imap_server, imap_port) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Integer, _>(r.user_id)
        .bind::<Text, _>(&r.method)
        .bind::<Text, _>(&r.encrypted_password)
        .bind::<Text, _>(&r.status)
        .bind::<Integer, _>(r.last_update)
        .bind::<Integer, _>(r.created_on)
        .bind::<Text, _>(&r.description)
        .bind::<Nullable<Text>, _>(&r.imap_server)
        .bind::<Nullable<Integer>, _>(r.imap_port)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error: {}", e),
        }
    }
    println!("imap_connection: migrated {} rows", count);
}

fn migrate_message_history(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Text)]
        role: String,
        #[diesel(sql_type = Text)]
        encrypted_content: String,
        #[diesel(sql_type = Nullable<Text>)]
        tool_name: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        tool_call_id: Option<String>,
        #[diesel(sql_type = Integer)]
        created_at: i32,
        #[diesel(sql_type = Text)]
        conversation_id: String,
        #[diesel(sql_type = Nullable<Text>)]
        tool_calls_json: Option<String>,
    }

    let rows: Vec<Row> = sql_query(
        "SELECT user_id, role, encrypted_content, tool_name, tool_call_id, \
         created_at, conversation_id, tool_calls_json FROM message_history",
    )
    .load(sqlite)
    .expect("Failed to read message_history");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO message_history (user_id, role, encrypted_content, tool_name, \
             tool_call_id, created_at, conversation_id, tool_calls_json) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind::<Integer, _>(r.user_id)
        .bind::<Text, _>(&r.role)
        .bind::<Text, _>(&r.encrypted_content)
        .bind::<Nullable<Text>, _>(&r.tool_name)
        .bind::<Nullable<Text>, _>(&r.tool_call_id)
        .bind::<Integer, _>(r.created_at)
        .bind::<Text, _>(&r.conversation_id)
        .bind::<Nullable<Text>, _>(&r.tool_calls_json)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error: {}", e),
        }
    }
    println!("message_history: migrated {} rows", count);
}

fn migrate_tesla(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Text)]
        encrypted_access_token: String,
        #[diesel(sql_type = Text)]
        encrypted_refresh_token: String,
        #[diesel(sql_type = Text)]
        status: String,
        #[diesel(sql_type = Integer)]
        last_update: i32,
        #[diesel(sql_type = Integer)]
        created_on: i32,
        #[diesel(sql_type = Integer)]
        expires_in: i32,
        #[diesel(sql_type = Text)]
        region: String,
        #[diesel(sql_type = Nullable<Text>)]
        selected_vehicle_vin: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        selected_vehicle_name: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        selected_vehicle_id: Option<String>,
        #[diesel(sql_type = Integer)]
        virtual_key_paired: i32,
        #[diesel(sql_type = Nullable<Text>)]
        granted_scopes: Option<String>,
    }

    let rows: Vec<Row> = sql_query(
        "SELECT id, user_id, encrypted_access_token, encrypted_refresh_token, status, \
         last_update, created_on, expires_in, region, selected_vehicle_vin, \
         selected_vehicle_name, selected_vehicle_id, virtual_key_paired, granted_scopes \
         FROM tesla",
    )
    .load(sqlite)
    .expect("Failed to read tesla");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO tesla (id, user_id, encrypted_access_token, encrypted_refresh_token, \
             status, last_update, created_on, expires_in, region, selected_vehicle_vin, \
             selected_vehicle_name, selected_vehicle_id, virtual_key_paired, granted_scopes) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Integer, _>(r.user_id)
        .bind::<Text, _>(&r.encrypted_access_token)
        .bind::<Text, _>(&r.encrypted_refresh_token)
        .bind::<Text, _>(&r.status)
        .bind::<Integer, _>(r.last_update)
        .bind::<Integer, _>(r.created_on)
        .bind::<Integer, _>(r.expires_in)
        .bind::<Text, _>(&r.region)
        .bind::<Nullable<Text>, _>(&r.selected_vehicle_vin)
        .bind::<Nullable<Text>, _>(&r.selected_vehicle_name)
        .bind::<Nullable<Text>, _>(&r.selected_vehicle_id)
        .bind::<Integer, _>(r.virtual_key_paired)
        .bind::<Nullable<Text>, _>(&r.granted_scopes)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error: {}", e),
        }
    }
    println!("tesla: migrated {} rows", count);
}

fn migrate_youtube(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Text)]
        encrypted_access_token: String,
        #[diesel(sql_type = Text)]
        encrypted_refresh_token: String,
        #[diesel(sql_type = Text)]
        status: String,
        #[diesel(sql_type = Integer)]
        expires_in: i32,
        #[diesel(sql_type = Integer)]
        last_update: i32,
        #[diesel(sql_type = Integer)]
        created_on: i32,
        #[diesel(sql_type = Text)]
        description: String,
    }

    let rows: Vec<Row> = sql_query(
        "SELECT id, user_id, encrypted_access_token, encrypted_refresh_token, status, \
         expires_in, last_update, created_on, description FROM youtube",
    )
    .load(sqlite)
    .expect("Failed to read youtube");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO youtube (id, user_id, encrypted_access_token, encrypted_refresh_token, \
             status, expires_in, last_update, created_on, description) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Integer, _>(r.user_id)
        .bind::<Text, _>(&r.encrypted_access_token)
        .bind::<Text, _>(&r.encrypted_refresh_token)
        .bind::<Text, _>(&r.status)
        .bind::<Integer, _>(r.expires_in)
        .bind::<Integer, _>(r.last_update)
        .bind::<Integer, _>(r.created_on)
        .bind::<Text, _>(&r.description)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error: {}", e),
        }
    }
    println!("youtube: migrated {} rows", count);
}

fn migrate_mcp_servers(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Text)]
        name: String,
        #[diesel(sql_type = Text)]
        url_encrypted: String,
        #[diesel(sql_type = Nullable<Text>)]
        auth_token_encrypted: Option<String>,
        #[diesel(sql_type = Integer)]
        is_enabled: i32,
        #[diesel(sql_type = Integer)]
        created_at: i32,
    }

    let rows: Vec<Row> = sql_query(
        "SELECT id, user_id, name, url_encrypted, auth_token_encrypted, is_enabled, created_at \
         FROM mcp_servers",
    )
    .load(sqlite)
    .expect("Failed to read mcp_servers");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO mcp_servers (id, user_id, name, url_encrypted, auth_token_encrypted, \
             is_enabled, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Integer, _>(r.user_id)
        .bind::<Text, _>(&r.name)
        .bind::<Text, _>(&r.url_encrypted)
        .bind::<Nullable<Text>, _>(&r.auth_token_encrypted)
        .bind::<Integer, _>(r.is_enabled)
        .bind::<Integer, _>(r.created_at)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error: {}", e),
        }
    }
    println!("mcp_servers: migrated {} rows", count);
}

fn migrate_totp_secrets(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Text)]
        encrypted_secret: String,
        #[diesel(sql_type = Integer)]
        enabled: i32,
        #[diesel(sql_type = Integer)]
        created_at: i32,
    }

    let rows: Vec<Row> =
        sql_query("SELECT id, user_id, encrypted_secret, enabled, created_at FROM totp_secrets")
            .load(sqlite)
            .expect("Failed to read totp_secrets");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO totp_secrets (id, user_id, encrypted_secret, enabled, created_at) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Integer, _>(r.user_id)
        .bind::<Text, _>(&r.encrypted_secret)
        .bind::<Integer, _>(r.enabled)
        .bind::<Integer, _>(r.created_at)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error: {}", e),
        }
    }
    println!("totp_secrets: migrated {} rows", count);
}

fn migrate_totp_backup_codes(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Text)]
        code_hash: String,
        #[diesel(sql_type = Integer)]
        used: i32,
    }

    let rows: Vec<Row> = sql_query("SELECT id, user_id, code_hash, used FROM totp_backup_codes")
        .load(sqlite)
        .expect("Failed to read totp_backup_codes");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO totp_backup_codes (id, user_id, code_hash, used) \
             VALUES ($1, $2, $3, $4) ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Integer, _>(r.user_id)
        .bind::<Text, _>(&r.code_hash)
        .bind::<Integer, _>(r.used)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error: {}", e),
        }
    }
    println!("totp_backup_codes: migrated {} rows", count);
}

fn migrate_webauthn_credentials(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Text)]
        credential_id: String,
        #[diesel(sql_type = Text)]
        encrypted_public_key: String,
        #[diesel(sql_type = Text)]
        device_name: String,
        #[diesel(sql_type = Integer)]
        counter: i32,
        #[diesel(sql_type = Nullable<Text>)]
        transports: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        aaguid: Option<String>,
        #[diesel(sql_type = Integer)]
        created_at: i32,
        #[diesel(sql_type = Nullable<Integer>)]
        last_used_at: Option<i32>,
        #[diesel(sql_type = Integer)]
        enabled: i32,
    }

    let rows: Vec<Row> = sql_query(
        "SELECT id, user_id, credential_id, encrypted_public_key, device_name, counter, \
         transports, aaguid, created_at, last_used_at, enabled FROM webauthn_credentials",
    )
    .load(sqlite)
    .expect("Failed to read webauthn_credentials");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO webauthn_credentials (id, user_id, credential_id, encrypted_public_key, \
             device_name, counter, transports, aaguid, created_at, last_used_at, enabled) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Integer, _>(r.user_id)
        .bind::<Text, _>(&r.credential_id)
        .bind::<Text, _>(&r.encrypted_public_key)
        .bind::<Text, _>(&r.device_name)
        .bind::<Integer, _>(r.counter)
        .bind::<Nullable<Text>, _>(&r.transports)
        .bind::<Nullable<Text>, _>(&r.aaguid)
        .bind::<Integer, _>(r.created_at)
        .bind::<Nullable<Integer>, _>(r.last_used_at)
        .bind::<Integer, _>(r.enabled)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error: {}", e),
        }
    }
    println!("webauthn_credentials: migrated {} rows", count);
}

fn migrate_webauthn_challenges(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Text)]
        challenge: String,
        #[diesel(sql_type = Text)]
        challenge_type: String,
        #[diesel(sql_type = Nullable<Text>)]
        context: Option<String>,
        #[diesel(sql_type = Integer)]
        created_at: i32,
        #[diesel(sql_type = Integer)]
        expires_at: i32,
    }

    let rows: Vec<Row> = sql_query(
        "SELECT id, user_id, challenge, challenge_type, context, created_at, expires_at \
         FROM webauthn_challenges",
    )
    .load(sqlite)
    .expect("Failed to read webauthn_challenges");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO webauthn_challenges (id, user_id, challenge, challenge_type, context, \
             created_at, expires_at) VALUES ($1, $2, $3, $4, $5, $6, $7) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Integer, _>(r.user_id)
        .bind::<Text, _>(&r.challenge)
        .bind::<Text, _>(&r.challenge_type)
        .bind::<Nullable<Text>, _>(&r.context)
        .bind::<Integer, _>(r.created_at)
        .bind::<Integer, _>(r.expires_at)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error: {}", e),
        }
    }
    println!("webauthn_challenges: migrated {} rows", count);
}

fn migrate_items(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Text)]
        summary: String,
        #[diesel(sql_type = Nullable<Integer>)]
        due_at: Option<i32>,
        #[diesel(sql_type = Integer)]
        priority: i32,
        #[diesel(sql_type = Nullable<Text>)]
        source_id: Option<String>,
        #[diesel(sql_type = Integer)]
        created_at: i32,
    }

    let rows: Vec<Row> = sql_query(
        "SELECT id, user_id, summary, due_at, priority, source_id, created_at FROM items",
    )
    .load(sqlite)
    .expect("Failed to read items");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO items (id, user_id, summary, due_at, priority, source_id, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Integer, _>(r.user_id)
        .bind::<Text, _>(&r.summary)
        .bind::<Nullable<Integer>, _>(r.due_at)
        .bind::<Integer, _>(r.priority)
        .bind::<Nullable<Text>, _>(&r.source_id)
        .bind::<Integer, _>(r.created_at)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error: {}", e),
        }
    }
    println!("items: migrated {} rows", count);
}

fn migrate_bridges(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Text)]
        bridge_type: String,
        #[diesel(sql_type = Text)]
        status: String,
        #[diesel(sql_type = Nullable<Text>)]
        room_id: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        data: Option<String>,
        #[diesel(sql_type = Nullable<Integer>)]
        created_at: Option<i32>,
        #[diesel(sql_type = Nullable<Integer>)]
        last_seen_online: Option<i32>,
    }

    let rows: Vec<Row> = sql_query(
        "SELECT id, user_id, bridge_type, status, room_id, data, created_at, last_seen_online FROM bridges",
    )
    .load(sqlite)
    .expect("Failed to read bridges");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO bridges (id, user_id, bridge_type, status, room_id, data, created_at, \
             last_seen_online) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Integer, _>(r.user_id)
        .bind::<Text, _>(&r.bridge_type)
        .bind::<Text, _>(&r.status)
        .bind::<Nullable<Text>, _>(&r.room_id)
        .bind::<Nullable<Text>, _>(&r.data)
        .bind::<Nullable<Integer>, _>(r.created_at)
        .bind::<Nullable<Integer>, _>(r.last_seen_online)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error: {}", e),
        }
    }
    println!("bridges: migrated {} rows", count);
}

fn migrate_bridge_disconnection_events(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Text)]
        bridge_type: String,
        #[diesel(sql_type = Integer)]
        detected_at: i32,
    }

    let rows: Vec<Row> =
        sql_query("SELECT id, user_id, bridge_type, detected_at FROM bridge_disconnection_events")
            .load(sqlite)
            .expect("Failed to read bridge_disconnection_events");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO bridge_disconnection_events (id, user_id, bridge_type, detected_at) \
             VALUES ($1, $2, $3, $4) ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Integer, _>(r.user_id)
        .bind::<Text, _>(&r.bridge_type)
        .bind::<Integer, _>(r.detected_at)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error: {}", e),
        }
    }
    println!("bridge_disconnection_events: migrated {} rows", count);
}

fn migrate_usage_logs(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Nullable<Text>)]
        sid: Option<String>,
        #[diesel(sql_type = Text)]
        activity_type: String,
        #[diesel(sql_type = Nullable<Float>)]
        credits: Option<f32>,
        #[diesel(sql_type = Integer)]
        created_at: i32,
        #[diesel(sql_type = Nullable<Integer>)]
        time_consumed: Option<i32>,
        #[diesel(sql_type = Nullable<Integer>)]
        success_int: Option<i32>,
        #[diesel(sql_type = Nullable<Text>)]
        reason: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        status: Option<String>,
        #[diesel(sql_type = Nullable<Integer>)]
        recharge_threshold_timestamp: Option<i32>,
        #[diesel(sql_type = Nullable<Integer>)]
        zero_credits_timestamp: Option<i32>,
        #[diesel(sql_type = Nullable<Integer>)]
        call_duration: Option<i32>,
    }

    let rows: Vec<Row> = sql_query(
        "SELECT user_id, sid, activity_type, credits, created_at, time_consumed, \
         success as success_int, reason, status, recharge_threshold_timestamp, \
         zero_credits_timestamp, call_duration FROM usage_logs",
    )
    .load(sqlite)
    .expect("Failed to read usage_logs");

    let mut count = 0;
    for r in &rows {
        // Convert SQLite integer boolean to PG boolean
        let success: Option<bool> = r.success_int.map(|v| v != 0);
        let result = diesel::sql_query(
            "INSERT INTO usage_logs (user_id, sid, activity_type, credits, created_at, \
             time_consumed, success, reason, status, recharge_threshold_timestamp, \
             zero_credits_timestamp, call_duration) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        )
        .bind::<Integer, _>(r.user_id)
        .bind::<Nullable<Text>, _>(&r.sid)
        .bind::<Text, _>(&r.activity_type)
        .bind::<Nullable<Float>, _>(r.credits)
        .bind::<Integer, _>(r.created_at)
        .bind::<Nullable<Integer>, _>(r.time_consumed)
        .bind::<Nullable<diesel::sql_types::Bool>, _>(success)
        .bind::<Nullable<Text>, _>(&r.reason)
        .bind::<Nullable<Text>, _>(&r.status)
        .bind::<Nullable<Integer>, _>(r.recharge_threshold_timestamp)
        .bind::<Nullable<Integer>, _>(r.zero_credits_timestamp)
        .bind::<Nullable<Integer>, _>(r.call_duration)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error: {}", e),
        }
    }
    println!("usage_logs: migrated {} rows", count);
}

fn migrate_processed_emails(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Text)]
        email_uid: String,
        #[diesel(sql_type = Integer)]
        processed_at: i32,
    }

    let rows: Vec<Row> = sql_query("SELECT user_id, email_uid, processed_at FROM processed_emails")
        .load(sqlite)
        .expect("Failed to read processed_emails");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO processed_emails (user_id, email_uid, processed_at) \
             VALUES ($1, $2, $3)",
        )
        .bind::<Integer, _>(r.user_id)
        .bind::<Text, _>(&r.email_uid)
        .bind::<Integer, _>(r.processed_at)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error: {}", e),
        }
    }
    println!("processed_emails: migrated {} rows", count);
}

fn reset_sequences(pg: &mut PgConnection) {
    let tables = [
        "contact_profiles",
        "contact_profile_exceptions",
        "message_history",
        "items",
        "bridges",
        "bridge_disconnection_events",
        "usage_logs",
        "imap_connection",
        "tesla",
        "youtube",
        "mcp_servers",
        "totp_secrets",
        "totp_backup_codes",
        "webauthn_credentials",
        "webauthn_challenges",
        "processed_emails",
    ];

    for table in &tables {
        let query = format!(
            "SELECT setval('{table}_id_seq', \
             (SELECT COALESCE(MAX(id), 1) FROM {table}), \
             (SELECT MAX(id) IS NOT NULL FROM {table}))"
        );
        let _ = sql_query(&query).execute(pg);
    }
    println!("sequences: reset all to current max(id)");
}
