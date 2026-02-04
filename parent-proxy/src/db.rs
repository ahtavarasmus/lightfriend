use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use std::sync::Mutex;
use tracing::debug;

/// Read-only database connection for looking up user routing info
pub struct Database {
    conn: Mutex<Connection>,
}

/// Enclave routing target
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnclaveTarget {
    Old,
    New,
}

impl Database {
    /// Open a read-only connection to the SQLite database
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("Failed to open database at {}", path))?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Look up which enclave to route a user to by phone number
    /// Returns New if user not found (new users go to new system)
    pub fn get_enclave_by_phone(&self, phone_number: &str) -> Result<EnclaveTarget> {
        let conn = self.conn.lock().unwrap();

        let result: Option<Option<String>> = conn
            .query_row(
                "SELECT active_enclave FROM users WHERE phone_number = ?",
                [phone_number],
                |row| row.get(0),
            )
            .optional()
            .context("Failed to query user by phone number")?;

        let target = match result {
            // User not found - route to new enclave (new users)
            None => {
                debug!(phone_number, "User not found, routing to new enclave");
                EnclaveTarget::New
            }
            // User found with NULL - route to new enclave (default)
            Some(None) => {
                debug!(phone_number, "User found with null enclave, routing to new");
                EnclaveTarget::New
            }
            // User found with "old" - route to old enclave
            Some(Some(ref s)) if s == "old" => {
                debug!(phone_number, "User found with old enclave, routing to old");
                EnclaveTarget::Old
            }
            // User found with "new" - route to new enclave
            Some(Some(ref s)) if s == "new" => {
                debug!(phone_number, "User found with new enclave, routing to new");
                EnclaveTarget::New
            }
            // Unknown value - default to old enclave for safety
            Some(Some(ref s)) => {
                debug!(phone_number, active_enclave = s, "Unknown enclave value, defaulting to old");
                EnclaveTarget::Old
            }
        };

        Ok(target)
    }

    /// Look up which enclave to route a user to by user ID
    /// Returns New if user not found (new users go to new system)
    pub fn get_enclave_by_user_id(&self, user_id: i32) -> Result<EnclaveTarget> {
        let conn = self.conn.lock().unwrap();

        let result: Option<Option<String>> = conn
            .query_row(
                "SELECT active_enclave FROM users WHERE id = ?",
                [user_id],
                |row| row.get(0),
            )
            .optional()
            .context("Failed to query user by ID")?;

        let target = match result {
            // User not found - route to new enclave (new users)
            None => {
                debug!(user_id, "User not found, routing to new enclave");
                EnclaveTarget::New
            }
            // User found with NULL - route to new enclave (default)
            Some(None) => {
                debug!(user_id, "User found with null enclave, routing to new");
                EnclaveTarget::New
            }
            // User found with "old" - route to old enclave
            Some(Some(ref s)) if s == "old" => {
                debug!(user_id, "User found with old enclave, routing to old");
                EnclaveTarget::Old
            }
            // User found with "new" - route to new enclave
            Some(Some(ref s)) if s == "new" => {
                debug!(user_id, "User found with new enclave, routing to new");
                EnclaveTarget::New
            }
            // Unknown value - default to old enclave for safety
            Some(Some(ref s)) => {
                debug!(user_id, active_enclave = s, "Unknown enclave value, defaulting to old");
                EnclaveTarget::Old
            }
        };

        Ok(target)
    }
}

/// Extension trait to add optional() to rusqlite results
trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for std::result::Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
