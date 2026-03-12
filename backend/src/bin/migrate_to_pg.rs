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
use diesel::sql_types::{Bool, Float, Integer, Nullable, Text};
use std::env;

fn main() {
    dotenvy::dotenv().ok();

    let sqlite_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pg_url = env::var("PG_DATABASE_URL").expect("PG_DATABASE_URL must be set");

    let mut sqlite_conn =
        SqliteConnection::establish(&sqlite_url).expect("Failed to connect to SQLite");
    let mut pg_conn = PgConnection::establish(&pg_url).expect("Failed to connect to PostgreSQL");

    println!("Connected to both databases. Starting migration...");

    // Migrate whole tables - users first (other tables reference it via FK)
    migrate_users(&mut sqlite_conn, &mut pg_conn);
    migrate_user_settings(&mut sqlite_conn, &mut pg_conn);
    migrate_waitlist(&mut sqlite_conn, &mut pg_conn);
    migrate_refund_info(&mut sqlite_conn, &mut pg_conn);
    migrate_message_status_log(&mut sqlite_conn, &mut pg_conn);
    migrate_country_availability(&mut sqlite_conn, &mut pg_conn);
    migrate_admin_alerts(&mut sqlite_conn, &mut pg_conn);
    migrate_disabled_alert_types(&mut sqlite_conn, &mut pg_conn);
    migrate_site_metrics(&mut sqlite_conn, &mut pg_conn);

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

fn migrate_users(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Text)]
        email: String,
        #[diesel(sql_type = Text)]
        password_hash: String,
        #[diesel(sql_type = Text)]
        phone_number: String,
        #[diesel(sql_type = Nullable<Text>)]
        nickname: Option<String>,
        #[diesel(sql_type = Nullable<Integer>)]
        time_to_live: Option<i32>,
        #[diesel(sql_type = Float)]
        credits: f32,
        #[diesel(sql_type = Nullable<Text>)]
        preferred_number: Option<String>,
        #[diesel(sql_type = Integer)]
        charge_when_under_int: i32,
        #[diesel(sql_type = Nullable<Float>)]
        charge_back_to: Option<f32>,
        #[diesel(sql_type = Nullable<Text>)]
        stripe_customer_id: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        stripe_payment_method_id: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        stripe_checkout_session_id: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        sub_tier: Option<String>,
        #[diesel(sql_type = Float)]
        credits_left: f32,
        #[diesel(sql_type = Nullable<Integer>)]
        last_credits_notification: Option<i32>,
        #[diesel(sql_type = Nullable<Integer>)]
        next_billing_date_timestamp: Option<i32>,
        #[diesel(sql_type = Nullable<Text>)]
        magic_token: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        plan_type: Option<String>,
        #[diesel(sql_type = Integer)]
        matrix_e2ee_enabled_int: i32,
    }

    let rows: Vec<Row> = sql_query(
        "SELECT id, email, password_hash, phone_number, nickname, time_to_live, credits, \
         preferred_number, charge_when_under AS charge_when_under_int, charge_back_to, \
         stripe_customer_id, stripe_payment_method_id, stripe_checkout_session_id, \
         sub_tier, credits_left, last_credits_notification, next_billing_date_timestamp, \
         magic_token, plan_type, matrix_e2ee_enabled AS matrix_e2ee_enabled_int FROM users",
    )
    .load(sqlite)
    .expect("Failed to read users");

    let mut count = 0;
    for r in &rows {
        let charge_when_under = r.charge_when_under_int != 0;
        let matrix_e2ee_enabled = r.matrix_e2ee_enabled_int != 0;
        let result = diesel::sql_query(
            "INSERT INTO users (id, email, password_hash, phone_number, nickname, time_to_live, \
             credits, preferred_number, charge_when_under, charge_back_to, stripe_customer_id, \
             stripe_payment_method_id, stripe_checkout_session_id, sub_tier, credits_left, \
             last_credits_notification, next_billing_date_timestamp, magic_token, plan_type, \
             matrix_e2ee_enabled) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Text, _>(&r.email)
        .bind::<Text, _>(&r.password_hash)
        .bind::<Text, _>(&r.phone_number)
        .bind::<Nullable<Text>, _>(&r.nickname)
        .bind::<Nullable<Integer>, _>(r.time_to_live)
        .bind::<Float, _>(r.credits)
        .bind::<Nullable<Text>, _>(&r.preferred_number)
        .bind::<Bool, _>(charge_when_under)
        .bind::<Nullable<Float>, _>(r.charge_back_to)
        .bind::<Nullable<Text>, _>(&r.stripe_customer_id)
        .bind::<Nullable<Text>, _>(&r.stripe_payment_method_id)
        .bind::<Nullable<Text>, _>(&r.stripe_checkout_session_id)
        .bind::<Nullable<Text>, _>(&r.sub_tier)
        .bind::<Float, _>(r.credits_left)
        .bind::<Nullable<Integer>, _>(r.last_credits_notification)
        .bind::<Nullable<Integer>, _>(r.next_billing_date_timestamp)
        .bind::<Nullable<Text>, _>(&r.magic_token)
        .bind::<Nullable<Text>, _>(&r.plan_type)
        .bind::<Bool, _>(matrix_e2ee_enabled)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error migrating users row: {}", e),
        }
    }
    println!("users: migrated {} rows", count);
}

fn migrate_user_settings(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Integer)]
        notify_int: i32,
        #[diesel(sql_type = Nullable<Text>)]
        notification_type: Option<String>,
        #[diesel(sql_type = Nullable<Integer>)]
        timezone_auto_int: Option<i32>,
        #[diesel(sql_type = Text)]
        agent_language: String,
        #[diesel(sql_type = Nullable<Text>)]
        sub_country: Option<String>,
        #[diesel(sql_type = Nullable<Integer>)]
        save_context: Option<i32>,
        #[diesel(sql_type = Nullable<Text>)]
        critical_enabled: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        elevenlabs_phone_number_id: Option<String>,
        #[diesel(sql_type = Integer)]
        notify_about_calls_int: i32,
        #[diesel(sql_type = Nullable<Text>)]
        action_on_critical_message: Option<String>,
        #[diesel(sql_type = Integer)]
        phone_service_active_int: i32,
        #[diesel(sql_type = Nullable<Text>)]
        default_notification_mode: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        default_notification_type: Option<String>,
        #[diesel(sql_type = Integer)]
        default_notify_on_call: i32,
        #[diesel(sql_type = Nullable<Text>)]
        llm_provider: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        phone_contact_notification_mode: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        phone_contact_notification_type: Option<String>,
        #[diesel(sql_type = Integer)]
        phone_contact_notify_on_call: i32,
        #[diesel(sql_type = Integer)]
        auto_create_items_int: i32,
    }

    let rows: Vec<Row> = sql_query(
        "SELECT id, user_id, notify AS notify_int, notification_type, \
         timezone_auto AS timezone_auto_int, agent_language, sub_country, save_context, \
         critical_enabled, elevenlabs_phone_number_id, \
         notify_about_calls AS notify_about_calls_int, action_on_critical_message, \
         phone_service_active AS phone_service_active_int, default_notification_mode, \
         default_notification_type, default_notify_on_call, llm_provider, \
         phone_contact_notification_mode, phone_contact_notification_type, \
         phone_contact_notify_on_call, auto_create_items AS auto_create_items_int \
         FROM user_settings",
    )
    .load(sqlite)
    .expect("Failed to read user_settings");

    let mut count = 0;
    for r in &rows {
        let notify = r.notify_int != 0;
        let timezone_auto: Option<bool> = r.timezone_auto_int.map(|v| v != 0);
        let notify_about_calls = r.notify_about_calls_int != 0;
        let phone_service_active = r.phone_service_active_int != 0;
        let auto_create_items = r.auto_create_items_int != 0;
        let result = diesel::sql_query(
            "INSERT INTO user_settings (id, user_id, notify, notification_type, timezone_auto, \
             agent_language, sub_country, save_context, critical_enabled, \
             elevenlabs_phone_number_id, notify_about_calls, action_on_critical_message, \
             phone_service_active, default_notification_mode, default_notification_type, \
             default_notify_on_call, llm_provider, phone_contact_notification_mode, \
             phone_contact_notification_type, phone_contact_notify_on_call, auto_create_items) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Integer, _>(r.user_id)
        .bind::<Bool, _>(notify)
        .bind::<Nullable<Text>, _>(&r.notification_type)
        .bind::<Nullable<Bool>, _>(timezone_auto)
        .bind::<Text, _>(&r.agent_language)
        .bind::<Nullable<Text>, _>(&r.sub_country)
        .bind::<Nullable<Integer>, _>(r.save_context)
        .bind::<Nullable<Text>, _>(&r.critical_enabled)
        .bind::<Nullable<Text>, _>(&r.elevenlabs_phone_number_id)
        .bind::<Bool, _>(notify_about_calls)
        .bind::<Nullable<Text>, _>(&r.action_on_critical_message)
        .bind::<Bool, _>(phone_service_active)
        .bind::<Nullable<Text>, _>(&r.default_notification_mode)
        .bind::<Nullable<Text>, _>(&r.default_notification_type)
        .bind::<Integer, _>(r.default_notify_on_call)
        .bind::<Nullable<Text>, _>(&r.llm_provider)
        .bind::<Nullable<Text>, _>(&r.phone_contact_notification_mode)
        .bind::<Nullable<Text>, _>(&r.phone_contact_notification_type)
        .bind::<Integer, _>(r.phone_contact_notify_on_call)
        .bind::<Bool, _>(auto_create_items)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error migrating user_settings row: {}", e),
        }
    }
    println!("user_settings: migrated {} rows", count);
}

fn migrate_waitlist(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Text)]
        email: String,
        #[diesel(sql_type = Integer)]
        created_at: i32,
    }

    let rows: Vec<Row> = sql_query("SELECT id, email, created_at FROM waitlist")
        .load(sqlite)
        .expect("Failed to read waitlist");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO waitlist (id, email, created_at) VALUES ($1, $2, $3) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Text, _>(&r.email)
        .bind::<Integer, _>(r.created_at)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error migrating waitlist row: {}", e),
        }
    }
    println!("waitlist: migrated {} rows", count);
}

fn migrate_refund_info(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Integer)]
        has_refunded: i32,
        #[diesel(sql_type = Nullable<Float>)]
        last_credit_pack_amount: Option<f32>,
        #[diesel(sql_type = Nullable<Integer>)]
        last_credit_pack_purchase_timestamp: Option<i32>,
        #[diesel(sql_type = Nullable<Integer>)]
        refunded_at: Option<i32>,
    }

    let rows: Vec<Row> = sql_query(
        "SELECT id, user_id, has_refunded, last_credit_pack_amount, \
         last_credit_pack_purchase_timestamp, refunded_at FROM refund_info",
    )
    .load(sqlite)
    .expect("Failed to read refund_info");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO refund_info (id, user_id, has_refunded, last_credit_pack_amount, \
             last_credit_pack_purchase_timestamp, refunded_at) \
             VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Integer, _>(r.user_id)
        .bind::<Integer, _>(r.has_refunded)
        .bind::<Nullable<Float>, _>(r.last_credit_pack_amount)
        .bind::<Nullable<Integer>, _>(r.last_credit_pack_purchase_timestamp)
        .bind::<Nullable<Integer>, _>(r.refunded_at)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error migrating refund_info row: {}", e),
        }
    }
    println!("refund_info: migrated {} rows", count);
}

fn migrate_message_status_log(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Text)]
        message_sid: String,
        #[diesel(sql_type = Integer)]
        user_id: i32,
        #[diesel(sql_type = Text)]
        direction: String,
        #[diesel(sql_type = Text)]
        to_number: String,
        #[diesel(sql_type = Nullable<Text>)]
        from_number: Option<String>,
        #[diesel(sql_type = Text)]
        status: String,
        #[diesel(sql_type = Nullable<Text>)]
        error_code: Option<String>,
        #[diesel(sql_type = Nullable<Text>)]
        error_message: Option<String>,
        #[diesel(sql_type = Integer)]
        created_at: i32,
        #[diesel(sql_type = Integer)]
        updated_at: i32,
        #[diesel(sql_type = Nullable<Float>)]
        price: Option<f32>,
        #[diesel(sql_type = Nullable<Text>)]
        price_unit: Option<String>,
    }

    let rows: Vec<Row> = sql_query(
        "SELECT id, message_sid, user_id, direction, to_number, from_number, status, \
         error_code, error_message, created_at, updated_at, price, price_unit \
         FROM message_status_log",
    )
    .load(sqlite)
    .expect("Failed to read message_status_log");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO message_status_log (id, message_sid, user_id, direction, to_number, \
             from_number, status, error_code, error_message, created_at, updated_at, price, \
             price_unit) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Text, _>(&r.message_sid)
        .bind::<Integer, _>(r.user_id)
        .bind::<Text, _>(&r.direction)
        .bind::<Text, _>(&r.to_number)
        .bind::<Nullable<Text>, _>(&r.from_number)
        .bind::<Text, _>(&r.status)
        .bind::<Nullable<Text>, _>(&r.error_code)
        .bind::<Nullable<Text>, _>(&r.error_message)
        .bind::<Integer, _>(r.created_at)
        .bind::<Integer, _>(r.updated_at)
        .bind::<Nullable<Float>, _>(r.price)
        .bind::<Nullable<Text>, _>(&r.price_unit)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error migrating message_status_log row: {}", e),
        }
    }
    println!("message_status_log: migrated {} rows", count);
}

fn migrate_country_availability(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Text)]
        country_code: String,
        #[diesel(sql_type = Integer)]
        has_local_numbers_int: i32,
        #[diesel(sql_type = Nullable<Float>)]
        outbound_sms_price: Option<f32>,
        #[diesel(sql_type = Nullable<Float>)]
        inbound_sms_price: Option<f32>,
        #[diesel(sql_type = Nullable<Float>)]
        outbound_voice_price_per_min: Option<f32>,
        #[diesel(sql_type = Nullable<Float>)]
        inbound_voice_price_per_min: Option<f32>,
        #[diesel(sql_type = Integer)]
        last_checked: i32,
        #[diesel(sql_type = Integer)]
        created_at: i32,
    }

    let rows: Vec<Row> = sql_query(
        "SELECT id, country_code, has_local_numbers AS has_local_numbers_int, \
         outbound_sms_price, inbound_sms_price, outbound_voice_price_per_min, \
         inbound_voice_price_per_min, last_checked, created_at FROM country_availability",
    )
    .load(sqlite)
    .expect("Failed to read country_availability");

    let mut count = 0;
    for r in &rows {
        let has_local_numbers = r.has_local_numbers_int != 0;
        let result = diesel::sql_query(
            "INSERT INTO country_availability (id, country_code, has_local_numbers, \
             outbound_sms_price, inbound_sms_price, outbound_voice_price_per_min, \
             inbound_voice_price_per_min, last_checked, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Text, _>(&r.country_code)
        .bind::<Bool, _>(has_local_numbers)
        .bind::<Nullable<Float>, _>(r.outbound_sms_price)
        .bind::<Nullable<Float>, _>(r.inbound_sms_price)
        .bind::<Nullable<Float>, _>(r.outbound_voice_price_per_min)
        .bind::<Nullable<Float>, _>(r.inbound_voice_price_per_min)
        .bind::<Integer, _>(r.last_checked)
        .bind::<Integer, _>(r.created_at)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error migrating country_availability row: {}", e),
        }
    }
    println!("country_availability: migrated {} rows", count);
}

fn migrate_admin_alerts(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Text)]
        alert_type: String,
        #[diesel(sql_type = Text)]
        severity: String,
        #[diesel(sql_type = Text)]
        message: String,
        #[diesel(sql_type = Text)]
        location: String,
        #[diesel(sql_type = Text)]
        module: String,
        #[diesel(sql_type = Integer)]
        acknowledged: i32,
        #[diesel(sql_type = Integer)]
        created_at: i32,
    }

    let rows: Vec<Row> = sql_query(
        "SELECT id, alert_type, severity, message, location, module, acknowledged, created_at \
         FROM admin_alerts",
    )
    .load(sqlite)
    .expect("Failed to read admin_alerts");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO admin_alerts (id, alert_type, severity, message, location, module, \
             acknowledged, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Text, _>(&r.alert_type)
        .bind::<Text, _>(&r.severity)
        .bind::<Text, _>(&r.message)
        .bind::<Text, _>(&r.location)
        .bind::<Text, _>(&r.module)
        .bind::<Integer, _>(r.acknowledged)
        .bind::<Integer, _>(r.created_at)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error migrating admin_alerts row: {}", e),
        }
    }
    println!("admin_alerts: migrated {} rows", count);
}

fn migrate_disabled_alert_types(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Text)]
        alert_type: String,
        #[diesel(sql_type = Integer)]
        disabled_at: i32,
    }

    let rows: Vec<Row> = sql_query("SELECT id, alert_type, disabled_at FROM disabled_alert_types")
        .load(sqlite)
        .expect("Failed to read disabled_alert_types");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO disabled_alert_types (id, alert_type, disabled_at) \
             VALUES ($1, $2, $3) ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Text, _>(&r.alert_type)
        .bind::<Integer, _>(r.disabled_at)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error migrating disabled_alert_types row: {}", e),
        }
    }
    println!("disabled_alert_types: migrated {} rows", count);
}

fn migrate_site_metrics(sqlite: &mut SqliteConnection, pg: &mut PgConnection) {
    #[derive(QueryableByName, Debug)]
    struct Row {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Text)]
        metric_key: String,
        #[diesel(sql_type = Text)]
        metric_value: String,
        #[diesel(sql_type = Integer)]
        updated_at: i32,
    }

    let rows: Vec<Row> =
        sql_query("SELECT id, metric_key, metric_value, updated_at FROM site_metrics")
            .load(sqlite)
            .expect("Failed to read site_metrics");

    let mut count = 0;
    for r in &rows {
        let result = diesel::sql_query(
            "INSERT INTO site_metrics (id, metric_key, metric_value, updated_at) \
             VALUES ($1, $2, $3, $4) ON CONFLICT (id) DO NOTHING",
        )
        .bind::<Integer, _>(r.id)
        .bind::<Text, _>(&r.metric_key)
        .bind::<Text, _>(&r.metric_value)
        .bind::<Integer, _>(r.updated_at)
        .execute(pg);

        match result {
            Ok(_) => count += 1,
            Err(e) => eprintln!("  Error migrating site_metrics row: {}", e),
        }
    }
    println!("site_metrics: migrated {} rows", count);
}

fn reset_sequences(pg: &mut PgConnection) {
    let tables = [
        "users",
        "user_settings",
        "waitlist",
        "refund_info",
        "message_status_log",
        "country_availability",
        "admin_alerts",
        "disabled_alert_types",
        "site_metrics",
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
