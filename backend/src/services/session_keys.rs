//! In-memory session key storage for encrypted backups.
//!
//! Session keys are derived from user's browser-generated master key and sent
//! to the enclave over HTTPS. They are held only in memory and never persisted.

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A session key for a single user's encrypted backup
#[derive(Clone)]
pub struct SessionKey {
    /// The 256-bit AES key for encrypting this user's data
    pub key: [u8; 32],
    /// When this key was established
    pub established_at: DateTime<Utc>,
    /// Session ID for tracking
    pub session_id: String,
}

/// Thread-safe in-memory storage for session keys
#[derive(Clone)]
pub struct SessionKeyStore {
    keys: Arc<RwLock<HashMap<i32, SessionKey>>>,
}

impl Default for SessionKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionKeyStore {
    /// Create a new empty session key store
    pub fn new() -> Self {
        Self {
            keys: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Store a session key for a user
    pub async fn set(&self, user_id: i32, key: [u8; 32], session_id: String) {
        let session_key = SessionKey {
            key,
            established_at: Utc::now(),
            session_id,
        };
        let mut keys = self.keys.write().await;
        keys.insert(user_id, session_key);
        tracing::info!("Session key established for user {}", user_id);
    }

    /// Get a session key for a user (if it exists)
    pub async fn get(&self, user_id: i32) -> Option<SessionKey> {
        let keys = self.keys.read().await;
        keys.get(&user_id).cloned()
    }

    /// Check if a user has an active session key
    pub async fn has_key(&self, user_id: i32) -> bool {
        let keys = self.keys.read().await;
        keys.contains_key(&user_id)
    }

    /// Remove a session key for a user
    pub async fn remove(&self, user_id: i32) {
        let mut keys = self.keys.write().await;
        if keys.remove(&user_id).is_some() {
            tracing::info!("Session key removed for user {}", user_id);
        }
    }

    /// Get all user IDs with active session keys
    pub async fn get_all_user_ids(&self) -> Vec<i32> {
        let keys = self.keys.read().await;
        keys.keys().copied().collect()
    }

    /// Get all session keys (for backup job)
    pub async fn get_all(&self) -> Vec<(i32, SessionKey)> {
        let keys = self.keys.read().await;
        keys.iter().map(|(id, key)| (*id, key.clone())).collect()
    }

    /// Get session info for a user (without exposing the key)
    pub async fn get_session_info(&self, user_id: i32) -> Option<SessionInfo> {
        let keys = self.keys.read().await;
        keys.get(&user_id).map(|sk| SessionInfo {
            session_id: sk.session_id.clone(),
            established_at: sk.established_at,
        })
    }
}

/// Public session info (does not expose the key)
#[derive(Clone, serde::Serialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub established_at: DateTime<Utc>,
}
