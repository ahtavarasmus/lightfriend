//! Bridge database backup service for encrypted backups.
//!
//! This module handles copying bridge data from PostgreSQL to an encrypted backup.
//! The process:
//! 1. Copy the ENTIRE whatsapp_db (all tables, all rows) to backup PostgreSQL database
//! 2. Go through the copy and encrypt sensitive values using each user's session key
//! 3. The backup DB has the exact same schema - just sensitive columns contain encrypted values
//!
//! Benefits:
//! - Maintains full database structure and referential integrity
//! - Same schema makes restore straightforward
//! - Non-sensitive columns remain queryable (timestamps, IDs, status flags)
//! - Only sensitive data (JIDs, message content, keys) is encrypted

use crate::repositories::user_core::UserCoreOps;
use crate::services::bridge_db_client::{
    get_all_tables, get_table_schema, table_exists, BridgeDbConnections, BridgeDbError, TableInfo,
};
use crate::services::bridge_encryption::{encrypt_string, SensitiveColumnConfig};
use crate::AppState;
use deadpool_postgres::Pool;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BridgeBackupError {
    #[error("Database connection error: {0}")]
    ConnectionError(String),
    #[error("Query error: {0}")]
    QueryError(String),
    #[error("Encryption error: {0}")]
    EncryptionError(#[from] crate::services::bridge_encryption::BridgeEncryptionError),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Environment error: {0}")]
    EnvError(String),
    #[error("No source database configured")]
    NoSourceDb,
    #[error("No backup database configured")]
    NoBackupDb,
    #[error("Database error: {0}")]
    DbError(#[from] BridgeDbError),
    #[error("Postgres error: {0}")]
    PostgresError(#[from] tokio_postgres::Error),
}

/// Configuration for bridge database backup
pub struct BridgeBackupConfig {
    /// PostgreSQL connection string for bridge database
    pub database_url: String,
    /// Directory to write encrypted backups
    pub output_dir: String,
    /// Sensitive column configuration
    pub sensitive_columns: SensitiveColumnConfig,
}

impl BridgeBackupConfig {
    /// Create config from environment variables
    pub fn from_env() -> Result<Self, BridgeBackupError> {
        let database_url = std::env::var("WHATSAPP_DB_URL")
            .map_err(|_| BridgeBackupError::EnvError("WHATSAPP_DB_URL not set".to_string()))?;

        let output_dir =
            std::env::var("ENCRYPTED_BACKUP_DIR").unwrap_or_else(|_| "/backup-storage".to_string());

        Ok(Self {
            database_url,
            output_dir,
            sensitive_columns: SensitiveColumnConfig::default(),
        })
    }
}

/// WhatsApp bridge tables that contain user data
/// Based on mautrix-whatsapp schema
pub const WHATSAPP_USER_TABLES: &[&str] = &[
    // User login table - maps Matrix users to WhatsApp JIDs
    "user_login",
    // Core user/device tables
    "whatsmeow_device",
    "whatsmeow_identity_key",
    "whatsmeow_pre_key",
    "whatsmeow_session",
    "whatsmeow_sender_key",
    "whatsmeow_app_state_sync_key",
    "whatsmeow_app_state_version",
    "whatsmeow_app_state_mutation_mac",
    // Contact and chat tables
    "whatsmeow_contact",
    "whatsmeow_chat_setting",
    // Message tables
    "message",
    "reaction",
    "disappearing_message",
    // Media tables
    "media_backfill_request",
    // Portal and puppet tables
    "portal",
    "puppet",
    "user_portal",
];

/// Columns to identify the user in each table
/// Maps table name to the column that contains the user's JID
pub const USER_JID_COLUMNS: &[(&str, &str)] = &[
    ("user_login", "id"), // id contains the JID like "+358..."
    ("whatsmeow_device", "jid"),
    ("whatsmeow_identity_key", "our_jid"),
    ("whatsmeow_pre_key", "jid"),
    ("whatsmeow_session", "our_jid"),
    ("whatsmeow_sender_key", "our_jid"),
    ("whatsmeow_app_state_sync_key", "jid"),
    ("whatsmeow_app_state_version", "jid"),
    ("whatsmeow_app_state_mutation_mac", "jid"),
    ("whatsmeow_contact", "our_jid"),
    ("whatsmeow_chat_setting", "our_jid"),
    ("message", "sender"),
    ("reaction", "sender"),
    ("disappearing_message", "room_id"),
    ("media_backfill_request", "user_mxid"),
];

/// Represents a row of data with column names and values
pub struct DataRow {
    pub columns: Vec<String>,
    pub values: Vec<Option<String>>,
}

/// Encrypt sensitive values in a data row
pub fn encrypt_row(
    row: &DataRow,
    session_key: &[u8; 32],
    config: &SensitiveColumnConfig,
) -> Result<DataRow, crate::services::bridge_encryption::BridgeEncryptionError> {
    let mut encrypted_values = Vec::with_capacity(row.values.len());

    for (i, value) in row.values.iter().enumerate() {
        let column_name = &row.columns[i];
        let encrypted = if let Some(v) = value {
            if config.is_sensitive(column_name) {
                Some(encrypt_string(session_key, v)?)
            } else {
                Some(v.clone())
            }
        } else {
            None
        };
        encrypted_values.push(encrypted);
    }

    Ok(DataRow {
        columns: row.columns.clone(),
        values: encrypted_values,
    })
}

/// Backup status for a single user
#[derive(Debug, Default)]
pub struct UserBackupStatus {
    pub tables_backed_up: usize,
    pub rows_encrypted: usize,
    pub errors: Vec<String>,
}

/// Overall backup statistics
#[derive(Debug, Default)]
pub struct BackupStats {
    pub tables_copied: usize,
    pub rows_copied: usize,
    pub tables_encrypted: usize,
    pub values_encrypted: usize,
    pub users_with_keys: usize,
    pub errors: Vec<String>,
}

/// Copy entire whatsapp_db to backup database
pub async fn copy_full_database(
    source: &Pool,
    backup: &Pool,
) -> Result<BackupStats, BridgeBackupError> {
    let mut stats = BackupStats::default();

    // Get list of all tables from source
    let tables = get_all_tables(source).await?;
    tracing::info!("Found {} tables to backup", tables.len());

    for table_name in tables {
        match copy_table(source, backup, &table_name).await {
            Ok(row_count) => {
                stats.tables_copied += 1;
                stats.rows_copied += row_count;
                tracing::debug!("Copied table {} ({} rows)", table_name, row_count);
            }
            Err(e) => {
                let error_msg = format!("Failed to copy table {}: {}", table_name, e);
                tracing::error!("{}", error_msg);
                stats.errors.push(error_msg);
            }
        }
    }

    Ok(stats)
}

/// Copy a single table from source to backup
async fn copy_table(
    source: &Pool,
    backup: &Pool,
    table_name: &str,
) -> Result<usize, BridgeBackupError> {
    // Get table schema
    let schema = get_table_schema(source, table_name).await?;

    // Create table in backup if it doesn't exist
    create_table_if_not_exists(backup, &schema).await?;

    // Truncate backup table (full refresh approach)
    truncate_table(backup, table_name).await?;

    // Copy all rows using raw SQL (simpler than dynamic parameter binding)
    let row_count = copy_table_data_raw(source, backup, &schema).await?;

    Ok(row_count)
}

/// Create table in backup database if it doesn't exist
async fn create_table_if_not_exists(
    backup: &Pool,
    schema: &TableInfo,
) -> Result<(), BridgeBackupError> {
    if table_exists(backup, &schema.name).await? {
        return Ok(());
    }

    let client = backup.get().await.map_err(BridgeDbError::from)?;

    // Build CREATE TABLE statement
    let mut columns_sql = Vec::new();
    for col in &schema.columns {
        let nullable = if col.is_nullable { "" } else { " NOT NULL" };
        columns_sql.push(format!("\"{}\" {}{}", col.name, col.data_type, nullable));
    }

    // Add primary key if exists
    let pk_clause = if !schema.primary_key.is_empty() {
        let pk_cols: Vec<String> = schema
            .primary_key
            .iter()
            .map(|c| format!("\"{}\"", c))
            .collect();
        format!(", PRIMARY KEY ({})", pk_cols.join(", "))
    } else {
        String::new()
    };

    let create_sql = format!(
        "CREATE TABLE IF NOT EXISTS \"{}\" ({}{})",
        schema.name,
        columns_sql.join(", "),
        pk_clause
    );

    client
        .execute(&create_sql, &[])
        .await
        .map_err(|e| BridgeBackupError::QueryError(format!("Failed to create table: {}", e)))?;

    tracing::debug!("Created table {} in backup database", schema.name);
    Ok(())
}

/// Truncate a table in the backup database
async fn truncate_table(backup: &Pool, table_name: &str) -> Result<(), BridgeBackupError> {
    let client = backup.get().await.map_err(BridgeDbError::from)?;
    let sql = format!("TRUNCATE TABLE \"{}\" CASCADE", table_name);
    client
        .execute(&sql, &[])
        .await
        .map_err(|e| BridgeBackupError::QueryError(format!("Failed to truncate table: {}", e)))?;
    Ok(())
}

/// Copy all data using INSERT ... SELECT via dblink or raw row-by-row copy
/// For simplicity, we use a row-by-row approach with string interpolation
/// This is safe because we're copying between trusted internal databases
async fn copy_table_data_raw(
    source: &Pool,
    backup: &Pool,
    schema: &TableInfo,
) -> Result<usize, BridgeBackupError> {
    let source_client = source.get().await.map_err(BridgeDbError::from)?;
    let backup_client = backup.get().await.map_err(BridgeDbError::from)?;

    // Build column list
    let columns: Vec<String> = schema
        .columns
        .iter()
        .map(|c| format!("\"{}\"", c.name))
        .collect();
    let columns_str = columns.join(", ");

    // Fetch all rows from source
    let select_sql = format!("SELECT {} FROM \"{}\"", columns_str, schema.name);
    let rows = source_client.query(&select_sql, &[]).await?;

    if rows.is_empty() {
        return Ok(0);
    }

    let mut row_count = 0;

    // Insert rows one by one, converting values to strings for simplicity
    for row in &rows {
        let mut values: Vec<String> = Vec::with_capacity(schema.columns.len());

        for (i, col) in schema.columns.iter().enumerate() {
            let value_str = format_column_value(row, i, &col.data_type);
            values.push(value_str);
        }

        let insert_sql = format!(
            "INSERT INTO \"{}\" ({}) VALUES ({})",
            schema.name,
            columns_str,
            values.join(", ")
        );

        match backup_client.execute(&insert_sql, &[]).await {
            Ok(_) => row_count += 1,
            Err(e) => {
                // Log but continue - some rows might fail due to constraints
                tracing::trace!("Failed to insert row into {}: {}", schema.name, e);
            }
        }
    }

    Ok(row_count)
}

/// Format a column value from a row for SQL insertion
fn format_column_value(row: &tokio_postgres::Row, col_index: usize, data_type: &str) -> String {
    // Try to get the value and format it appropriately
    // Return NULL if the value is null or we can't extract it

    match data_type {
        "integer" | "int4" => {
            if let Ok(Some(val)) = row.try_get::<_, Option<i32>>(col_index) {
                val.to_string()
            } else {
                "NULL".to_string()
            }
        }
        "bigint" | "int8" => {
            if let Ok(Some(val)) = row.try_get::<_, Option<i64>>(col_index) {
                val.to_string()
            } else {
                "NULL".to_string()
            }
        }
        "smallint" | "int2" => {
            if let Ok(Some(val)) = row.try_get::<_, Option<i16>>(col_index) {
                val.to_string()
            } else {
                "NULL".to_string()
            }
        }
        "boolean" | "bool" => {
            if let Ok(Some(val)) = row.try_get::<_, Option<bool>>(col_index) {
                if val { "TRUE" } else { "FALSE" }.to_string()
            } else {
                "NULL".to_string()
            }
        }
        "real" | "float4" => {
            if let Ok(Some(val)) = row.try_get::<_, Option<f32>>(col_index) {
                val.to_string()
            } else {
                "NULL".to_string()
            }
        }
        "double precision" | "float8" => {
            if let Ok(Some(val)) = row.try_get::<_, Option<f64>>(col_index) {
                val.to_string()
            } else {
                "NULL".to_string()
            }
        }
        "bytea" => {
            if let Ok(Some(val)) = row.try_get::<_, Option<Vec<u8>>>(col_index) {
                format!("'\\x{}'", hex::encode(val))
            } else {
                "NULL".to_string()
            }
        }
        "json" | "jsonb" => {
            if let Ok(Some(val)) = row.try_get::<_, Option<serde_json::Value>>(col_index) {
                format!("'{}'", val.to_string().replace('\'', "''"))
            } else {
                "NULL".to_string()
            }
        }
        "timestamp" | "timestamp without time zone" => {
            if let Ok(Some(val)) = row.try_get::<_, Option<chrono::NaiveDateTime>>(col_index) {
                format!("'{}'", val.format("%Y-%m-%d %H:%M:%S%.6f"))
            } else {
                "NULL".to_string()
            }
        }
        "timestamp with time zone" | "timestamptz" => {
            if let Ok(Some(val)) =
                row.try_get::<_, Option<chrono::DateTime<chrono::Utc>>>(col_index)
            {
                format!("'{}'", val.format("%Y-%m-%d %H:%M:%S%.6f%:z"))
            } else {
                "NULL".to_string()
            }
        }
        _ => {
            // Default to text/string for unknown types
            if let Ok(Some(val)) = row.try_get::<_, Option<String>>(col_index) {
                format!("'{}'", val.replace('\'', "''"))
            } else {
                "NULL".to_string()
            }
        }
    }
}

/// Build mapping from JID to session_key
/// Uses: user_id -> matrix_username -> user_mxid -> JID
pub async fn build_jid_session_key_map(
    app_state: &Arc<AppState>,
    backup_pool: &Pool,
) -> HashMap<String, [u8; 32]> {
    let mut map = HashMap::new();

    // Get all users with active session keys
    let session_keys = app_state.session_key_store.get_all().await;

    for (user_id, session_key) in session_keys {
        // Get user's matrix_username from SQLite
        if let Ok(Some(user)) = app_state.user_core.find_by_id(user_id) {
            if let Some(matrix_username) = &user.matrix_username {
                // Look up JID from backup DB's user_login table
                match get_jid_for_matrix_user(backup_pool, matrix_username).await {
                    Ok(Some(jid)) => {
                        map.insert(jid.clone(), session_key.key);
                        tracing::debug!(
                            "Mapped user {} (matrix: {}) to JID {}",
                            user_id,
                            matrix_username,
                            jid
                        );
                    }
                    Ok(None) => {
                        tracing::debug!(
                            "No JID found for matrix user {} (user_id: {})",
                            matrix_username,
                            user_id
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to look up JID for matrix user {}: {}",
                            matrix_username,
                            e
                        );
                    }
                }
            }
        }
    }

    map
}

/// Look up JID for a Matrix user from the user_login table
async fn get_jid_for_matrix_user(
    pool: &Pool,
    matrix_username: &str,
) -> Result<Option<String>, BridgeBackupError> {
    let client = pool.get().await.map_err(BridgeDbError::from)?;

    // The user_login table has:
    // - user_mxid: Matrix user ID (e.g., "@username:localhost")
    // - id: The WhatsApp JID (e.g., "+358442105886@s.whatsapp.net")

    // Matrix username might be just "username" or "@username:localhost"
    let user_mxid = if matrix_username.starts_with('@') {
        matrix_username.to_string()
    } else {
        format!("@{}:localhost", matrix_username)
    };

    let rows = client
        .query(
            "SELECT id FROM user_login WHERE user_mxid = $1",
            &[&user_mxid],
        )
        .await?;

    Ok(rows.first().map(|row| row.get(0)))
}

/// Encrypt sensitive columns in the backup database
pub async fn encrypt_backup_sensitive_values(
    backup: &Pool,
    jid_keys: &HashMap<String, [u8; 32]>,
    config: &SensitiveColumnConfig,
) -> Result<BackupStats, BridgeBackupError> {
    let mut stats = BackupStats {
        users_with_keys: jid_keys.len(),
        ..Default::default()
    };

    if jid_keys.is_empty() {
        tracing::info!("No session keys available, skipping encryption");
        return Ok(stats);
    }

    // Process each table that has user data
    for (table_name, jid_column) in USER_JID_COLUMNS {
        if !table_exists(backup, table_name).await? {
            continue;
        }

        match encrypt_table_sensitive_columns(backup, table_name, jid_column, jid_keys, config)
            .await
        {
            Ok(encrypted_count) => {
                if encrypted_count > 0 {
                    stats.tables_encrypted += 1;
                    stats.values_encrypted += encrypted_count;
                    tracing::debug!(
                        "Encrypted {} values in table {}",
                        encrypted_count,
                        table_name
                    );
                }
            }
            Err(e) => {
                let error_msg = format!("Failed to encrypt table {}: {}", table_name, e);
                tracing::error!("{}", error_msg);
                stats.errors.push(error_msg);
            }
        }
    }

    Ok(stats)
}

/// Encrypt sensitive columns in a single table
async fn encrypt_table_sensitive_columns(
    backup: &Pool,
    table_name: &str,
    jid_column: &str,
    jid_keys: &HashMap<String, [u8; 32]>,
    config: &SensitiveColumnConfig,
) -> Result<usize, BridgeBackupError> {
    let schema = get_table_schema(backup, table_name).await?;
    let client = backup.get().await.map_err(BridgeDbError::from)?;

    // Find which columns are sensitive
    let sensitive_cols: Vec<&String> = schema
        .columns
        .iter()
        .filter(|c| config.is_sensitive(&c.name))
        .map(|c| &c.name)
        .collect();

    if sensitive_cols.is_empty() {
        return Ok(0);
    }

    // Build select query for JID column + sensitive columns
    let select_cols: Vec<String> = std::iter::once(format!("\"{}\"", jid_column))
        .chain(sensitive_cols.iter().map(|c| format!("\"{}\"", c)))
        .collect();

    let select_sql = format!("SELECT {} FROM \"{}\"", select_cols.join(", "), table_name);
    let rows = client.query(&select_sql, &[]).await?;

    let mut encrypted_count = 0;

    for row in &rows {
        // Get the JID value to look up the session key
        let jid: Option<String> = row.try_get(0).ok().flatten();

        let Some(jid) = jid else {
            continue;
        };

        // Look up session key for this JID
        let Some(session_key) = jid_keys.get(&jid) else {
            // No session key for this user - skip
            continue;
        };

        // Encrypt each sensitive column and update
        for (i, col_name) in sensitive_cols.iter().enumerate() {
            let col_index = i + 1; // +1 because JID is at index 0

            let value: Option<String> = row.try_get(col_index).ok().flatten();

            if let Some(plaintext) = value {
                // Encrypt the value
                let encrypted = encrypt_string(session_key, &plaintext)?;

                // Build UPDATE query
                let update_sql = format!(
                    "UPDATE \"{}\" SET \"{}\" = $1 WHERE \"{}\" = $2",
                    table_name, col_name, jid_column
                );

                if let Err(e) = client.execute(&update_sql, &[&encrypted, &jid]).await {
                    tracing::trace!("Failed to update {}.{}: {}", table_name, col_name, e);
                } else {
                    encrypted_count += 1;
                }
            }
        }
    }

    Ok(encrypted_count)
}

/// Main backup orchestration function
/// Run full bridge database backup with encryption
pub async fn run_bridge_backup(
    app_state: &Arc<AppState>,
) -> Result<BackupStats, BridgeBackupError> {
    tracing::info!("Starting bridge database backup...");

    let connections = BridgeDbConnections::connect().await.map_err(|e| {
        BridgeBackupError::ConnectionError(format!("Failed to connect to databases: {}", e))
    })?;

    let source = connections
        .require_source()
        .map_err(|_| BridgeBackupError::NoSourceDb)?;
    let backup = connections
        .require_backup()
        .map_err(|_| BridgeBackupError::NoBackupDb)?;

    // Step 1: Copy entire database
    tracing::info!("Copying whatsapp_db to backup...");
    let mut stats = copy_full_database(source, backup).await?;
    tracing::info!(
        "Database copy complete: {} tables, {} rows",
        stats.tables_copied,
        stats.rows_copied
    );

    // Step 2: Build JID -> session_key mapping
    let jid_keys = build_jid_session_key_map(app_state, backup).await;
    tracing::info!("Found {} users with session keys", jid_keys.len());

    // Step 3: Encrypt sensitive values in backup
    if !jid_keys.is_empty() {
        tracing::info!("Encrypting sensitive values...");
        let config = SensitiveColumnConfig::default();
        let encryption_stats = encrypt_backup_sensitive_values(backup, &jid_keys, &config).await?;

        stats.tables_encrypted = encryption_stats.tables_encrypted;
        stats.values_encrypted = encryption_stats.values_encrypted;
        stats.users_with_keys = encryption_stats.users_with_keys;
        stats.errors.extend(encryption_stats.errors);
    }

    tracing::info!(
        "Bridge backup complete: {} tables copied, {} rows, {} values encrypted for {} users",
        stats.tables_copied,
        stats.rows_copied,
        stats.values_encrypted,
        stats.users_with_keys
    );

    Ok(stats)
}

/// Backup all bridge data for a user (legacy placeholder, now using full database copy)
pub async fn backup_user_bridge_data(
    _user_id: i32,
    _user_jid: &str,
    _session_key: &[u8; 32],
    _config: &BridgeBackupConfig,
) -> Result<UserBackupStatus, BridgeBackupError> {
    // This function is kept for backward compatibility
    // The new approach uses run_bridge_backup() which does full database copy + encrypt
    tracing::debug!("backup_user_bridge_data called - use run_bridge_backup() for full backup");
    Ok(UserBackupStatus::default())
}

/// Get the database URL for a specific bridge type
pub fn get_bridge_db_url(bridge_type: &str) -> Option<String> {
    match bridge_type {
        "whatsapp" => std::env::var("WHATSAPP_DB_URL").ok(),
        "telegram" => std::env::var("TELEGRAM_DB_URL").ok(),
        "signal" => std::env::var("SIGNAL_DB_URL").ok(),
        "messenger" => std::env::var("MESSENGER_DB_URL").ok(),
        "instagram" => std::env::var("INSTAGRAM_DB_URL").ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::bridge_encryption::SensitiveColumnConfig;

    #[test]
    fn test_encrypt_row() {
        let row = DataRow {
            columns: vec![
                "id".to_string(),
                "sender_jid".to_string(),
                "message".to_string(),
                "created_at".to_string(),
            ],
            values: vec![
                Some("1".to_string()),
                Some("+15551234567@s.whatsapp.net".to_string()),
                Some("Hello, world!".to_string()),
                Some("2024-01-01".to_string()),
            ],
        };

        let session_key: [u8; 32] = [42u8; 32];
        let config = SensitiveColumnConfig::default();

        let encrypted = encrypt_row(&row, &session_key, &config).unwrap();

        // id should be unchanged (not sensitive)
        assert_eq!(encrypted.values[0], Some("1".to_string()));

        // sender_jid should be encrypted (contains "jid")
        assert_ne!(
            encrypted.values[1],
            Some("+15551234567@s.whatsapp.net".to_string())
        );
        assert!(encrypted.values[1].as_ref().unwrap().len() > 20); // Encrypted is longer

        // created_at should be unchanged (not sensitive)
        assert_eq!(encrypted.values[3], Some("2024-01-01".to_string()));
    }

    #[test]
    fn test_backup_stats_default() {
        let stats = BackupStats::default();
        assert_eq!(stats.tables_copied, 0);
        assert_eq!(stats.rows_copied, 0);
        assert_eq!(stats.values_encrypted, 0);
        assert!(stats.errors.is_empty());
    }
}
