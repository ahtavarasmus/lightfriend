//! PostgreSQL connection manager for bridge databases.
//!
//! Manages connections to:
//! - Source database (whatsapp_db inside enclave) - contains live bridge data
//! - Backup database (whatsapp_backup_db on external storage) - encrypted backup copy

use deadpool_postgres::{Config, Pool, Runtime};
use std::time::Duration;
use thiserror::Error;
use tokio_postgres::NoTls;

#[derive(Error, Debug)]
pub enum BridgeDbError {
    #[error("Database connection error: {0}")]
    ConnectionError(String),
    #[error("Pool error: {0}")]
    PoolError(#[from] deadpool_postgres::PoolError),
    #[error("Create pool error: {0}")]
    CreatePoolError(#[from] deadpool_postgres::CreatePoolError),
    #[error("Postgres error: {0}")]
    PostgresError(#[from] tokio_postgres::Error),
    #[error("Environment variable not set: {0}")]
    EnvError(String),
    #[error("No source database configured")]
    NoSourceDb,
    #[error("No backup database configured")]
    NoBackupDb,
}

/// Manages PostgreSQL connections to source and backup bridge databases
pub struct BridgeDbConnections {
    source_pool: Option<Pool>,
    backup_pool: Option<Pool>,
}

/// Parse a PostgreSQL connection URL into deadpool config
fn parse_pg_url(url: &str) -> Result<Config, BridgeDbError> {
    // Format: postgres://user:password@host:port/database
    let url = url::Url::parse(url)
        .map_err(|e| BridgeDbError::ConnectionError(format!("Invalid URL: {}", e)))?;

    let mut cfg = Config::new();
    cfg.host = url.host_str().map(String::from);
    cfg.port = url.port();
    cfg.user = if url.username().is_empty() {
        None
    } else {
        Some(url.username().to_string())
    };
    cfg.password = url.password().map(String::from);
    cfg.dbname = Some(url.path().trim_start_matches('/').to_string());

    Ok(cfg)
}

impl BridgeDbConnections {
    /// Create connection pools for source and backup databases.
    /// Returns Ok even if one or both URLs are not configured (pools will be None).
    pub async fn connect() -> Result<Self, BridgeDbError> {
        let source_url = std::env::var("WHATSAPP_DB_URL").ok();
        let backup_url = std::env::var("WHATSAPP_BACKUP_DB_URL").ok();

        let source_pool = if let Some(url) = source_url {
            tracing::debug!("Connecting to source bridge database...");
            match Self::create_pool(&url).await {
                Ok(pool) => {
                    tracing::info!("Connected to source bridge database");
                    Some(pool)
                }
                Err(e) => {
                    tracing::warn!("Failed to connect to source bridge database: {}", e);
                    None
                }
            }
        } else {
            tracing::debug!("WHATSAPP_DB_URL not set, source database not available");
            None
        };

        let backup_pool = if let Some(url) = backup_url {
            if url.is_empty() {
                tracing::debug!("WHATSAPP_BACKUP_DB_URL is empty, backup database not available");
                None
            } else {
                tracing::debug!("Connecting to backup bridge database...");
                match Self::create_pool(&url).await {
                    Ok(pool) => {
                        tracing::info!("Connected to backup bridge database");
                        Some(pool)
                    }
                    Err(e) => {
                        tracing::warn!("Failed to connect to backup bridge database: {}", e);
                        None
                    }
                }
            }
        } else {
            tracing::debug!("WHATSAPP_BACKUP_DB_URL not set, backup database not available");
            None
        };

        Ok(Self {
            source_pool,
            backup_pool,
        })
    }

    async fn create_pool(url: &str) -> Result<Pool, BridgeDbError> {
        let mut cfg = parse_pg_url(url)?;
        cfg.connect_timeout = Some(Duration::from_secs(10));

        let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls)?;

        // Test the connection
        let client = pool.get().await?;
        client.simple_query("SELECT 1").await?;

        Ok(pool)
    }

    /// Get reference to source database pool (whatsapp_db in enclave)
    pub fn source(&self) -> Option<&Pool> {
        self.source_pool.as_ref()
    }

    /// Get reference to backup database pool (whatsapp_backup_db external)
    pub fn backup(&self) -> Option<&Pool> {
        self.backup_pool.as_ref()
    }

    /// Check if both databases are connected
    pub fn is_fully_connected(&self) -> bool {
        self.source_pool.is_some() && self.backup_pool.is_some()
    }

    /// Get source pool or return error
    pub fn require_source(&self) -> Result<&Pool, BridgeDbError> {
        self.source_pool.as_ref().ok_or(BridgeDbError::NoSourceDb)
    }

    /// Get backup pool or return error
    pub fn require_backup(&self) -> Result<&Pool, BridgeDbError> {
        self.backup_pool.as_ref().ok_or(BridgeDbError::NoBackupDb)
    }
}

/// Table metadata for PostgreSQL operations
#[derive(Debug, Clone)]
pub struct TableInfo {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
    pub primary_key: Vec<String>,
}

/// Column metadata
#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
}

/// Get list of all tables in the database
pub async fn get_all_tables(pool: &Pool) -> Result<Vec<String>, BridgeDbError> {
    let client = pool.get().await?;

    let rows = client
        .query(
            r#"
            SELECT table_name
            FROM information_schema.tables
            WHERE table_schema = 'public'
            AND table_type = 'BASE TABLE'
            ORDER BY table_name
            "#,
            &[],
        )
        .await?;

    Ok(rows.into_iter().map(|row| row.get(0)).collect())
}

/// Get schema for a specific table
pub async fn get_table_schema(pool: &Pool, table_name: &str) -> Result<TableInfo, BridgeDbError> {
    let client = pool.get().await?;

    // Get column information
    let rows = client
        .query(
            r#"
            SELECT column_name, data_type, is_nullable
            FROM information_schema.columns
            WHERE table_schema = 'public' AND table_name = $1
            ORDER BY ordinal_position
            "#,
            &[&table_name],
        )
        .await?;

    let column_infos: Vec<ColumnInfo> = rows
        .into_iter()
        .map(|row| {
            let name: String = row.get(0);
            let data_type: String = row.get(1);
            let nullable: String = row.get(2);
            ColumnInfo {
                name,
                data_type,
                is_nullable: nullable == "YES",
            }
        })
        .collect();

    // Get primary key columns
    let pk_rows = client
        .query(
            r#"
            SELECT a.attname
            FROM pg_index i
            JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
            WHERE i.indrelid = $1::regclass
            AND i.indisprimary
            "#,
            &[&table_name],
        )
        .await
        .unwrap_or_default();

    let primary_key: Vec<String> = pk_rows.into_iter().map(|row| row.get(0)).collect();

    Ok(TableInfo {
        name: table_name.to_string(),
        columns: column_infos,
        primary_key,
    })
}

/// Check if a table exists in the database
pub async fn table_exists(pool: &Pool, table_name: &str) -> Result<bool, BridgeDbError> {
    let client = pool.get().await?;

    let row = client
        .query_one(
            r#"
            SELECT COUNT(*)
            FROM information_schema.tables
            WHERE table_schema = 'public' AND table_name = $1
            "#,
            &[&table_name],
        )
        .await?;

    let count: i64 = row.get(0);
    Ok(count > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_info() {
        let col = ColumnInfo {
            name: "test".to_string(),
            data_type: "text".to_string(),
            is_nullable: true,
        };
        assert_eq!(col.name, "test");
        assert!(col.is_nullable);
    }

    #[test]
    fn test_table_info() {
        let table = TableInfo {
            name: "users".to_string(),
            columns: vec![],
            primary_key: vec!["id".to_string()],
        };
        assert_eq!(table.name, "users");
        assert_eq!(table.primary_key.len(), 1);
    }

    #[test]
    fn test_parse_pg_url() {
        let cfg = parse_pg_url("postgres://user:pass@localhost:5432/mydb").unwrap();
        assert_eq!(cfg.host, Some("localhost".to_string()));
        assert_eq!(cfg.port, Some(5432));
        assert_eq!(cfg.user, Some("user".to_string()));
        assert_eq!(cfg.password, Some("pass".to_string()));
        assert_eq!(cfg.dbname, Some("mydb".to_string()));
    }
}
