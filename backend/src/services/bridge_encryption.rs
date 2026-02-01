//! Bridge data encryption service for encrypted backups.
//!
//! This module provides per-user encryption of sensitive bridge data columns
//! using their session key. The encryption is AES-256-GCM.
//!
//! Sensitive columns that get encrypted:
//! - Phone numbers: *_jid columns (sender_jid, chat_jid, entity_jid)
//! - Message content: data columns (protobuf message bodies)
//! - Encryption keys: whatsmeow_* key columns (identity_key, pre_key, etc.)
//! - Media keys: media_key columns

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::Rng;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BridgeEncryptionError {
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),
    #[error("Invalid key length")]
    InvalidKeyLength,
    #[error("Invalid data format")]
    InvalidDataFormat,
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("No session key for user")]
    NoSessionKey,
}

/// Encrypt a value using the user's session key
///
/// Returns base64-encoded ciphertext with prepended nonce
pub fn encrypt_value(
    session_key: &[u8; 32],
    plaintext: &[u8],
) -> Result<String, BridgeEncryptionError> {
    let cipher = Aes256Gcm::new_from_slice(session_key)
        .map_err(|e| BridgeEncryptionError::EncryptionFailed(e.to_string()))?;

    // Generate random nonce
    let mut rng = rand::thread_rng();
    let mut nonce_bytes = [0u8; 12];
    rng.fill(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| BridgeEncryptionError::EncryptionFailed(e.to_string()))?;

    // Combine nonce + ciphertext
    let mut combined = nonce_bytes.to_vec();
    combined.extend(ciphertext);

    Ok(BASE64.encode(combined))
}

/// Decrypt a value using the user's session key
///
/// Expects base64-encoded data with prepended nonce
pub fn decrypt_value(
    session_key: &[u8; 32],
    encrypted: &str,
) -> Result<Vec<u8>, BridgeEncryptionError> {
    let cipher = Aes256Gcm::new_from_slice(session_key)
        .map_err(|e| BridgeEncryptionError::DecryptionFailed(e.to_string()))?;

    // Decode base64
    let encrypted_data = BASE64
        .decode(encrypted)
        .map_err(|_| BridgeEncryptionError::InvalidDataFormat)?;

    // Extract nonce and ciphertext
    if encrypted_data.len() < 12 {
        return Err(BridgeEncryptionError::InvalidDataFormat);
    }

    let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    // Decrypt
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| BridgeEncryptionError::DecryptionFailed(e.to_string()))
}

/// Encrypt a string value, returning encrypted base64
pub fn encrypt_string(
    session_key: &[u8; 32],
    plaintext: &str,
) -> Result<String, BridgeEncryptionError> {
    encrypt_value(session_key, plaintext.as_bytes())
}

/// Decrypt to a string
pub fn decrypt_string(
    session_key: &[u8; 32],
    encrypted: &str,
) -> Result<String, BridgeEncryptionError> {
    let bytes = decrypt_value(session_key, encrypted)?;
    String::from_utf8(bytes).map_err(|_| BridgeEncryptionError::InvalidDataFormat)
}

/// Configuration for which columns are considered sensitive
pub struct SensitiveColumnConfig {
    /// Column names that contain phone numbers / JIDs
    pub jid_columns: Vec<String>,
    /// Column names that contain message content / data blobs
    pub data_columns: Vec<String>,
    /// Column names that contain encryption keys
    pub key_columns: Vec<String>,
}

impl Default for SensitiveColumnConfig {
    fn default() -> Self {
        Self {
            jid_columns: vec![
                "our_jid".to_string(),
                "their_jid".to_string(),
                "jid".to_string(),
                "sender".to_string(),
                "chat_jid".to_string(),
                "sender_jid".to_string(),
                "recipient".to_string(),
            ],
            data_columns: vec![
                "data".to_string(),
                "message".to_string(),
                "body".to_string(),
                "content".to_string(),
                "media_key".to_string(),
            ],
            key_columns: vec![
                "identity".to_string(),
                "key".to_string(),
                "private_key".to_string(),
                "public_key".to_string(),
                "registration_id".to_string(),
            ],
        }
    }
}

impl SensitiveColumnConfig {
    /// Check if a column name is sensitive
    pub fn is_sensitive(&self, column_name: &str) -> bool {
        let lower = column_name.to_lowercase();

        // Check exact matches
        if self.jid_columns.iter().any(|c| lower == c.to_lowercase()) {
            return true;
        }
        if self.data_columns.iter().any(|c| lower == c.to_lowercase()) {
            return true;
        }
        if self.key_columns.iter().any(|c| lower == c.to_lowercase()) {
            return true;
        }

        // Check suffix patterns (e.g., "sender_jid", "media_key")
        for pattern in &self.jid_columns {
            if lower.ends_with(&format!("_{}", pattern.to_lowercase())) {
                return true;
            }
        }
        for pattern in &self.key_columns {
            if lower.ends_with(&format!("_{}", pattern.to_lowercase())) {
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key: [u8; 32] = [42u8; 32];
        let plaintext = "Hello, World!";

        let encrypted = encrypt_string(&key, plaintext).unwrap();
        let decrypted = decrypt_string(&key, &encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_encrypt_decrypt_binary() {
        let key: [u8; 32] = [42u8; 32];
        let data: Vec<u8> = vec![0, 1, 2, 255, 254, 253];

        let encrypted = encrypt_value(&key, &data).unwrap();
        let decrypted = decrypt_value(&key, &encrypted).unwrap();

        assert_eq!(data, decrypted);
    }

    #[test]
    fn test_different_keys_produce_different_ciphertext() {
        let key1: [u8; 32] = [1u8; 32];
        let key2: [u8; 32] = [2u8; 32];
        let plaintext = "same message";

        let encrypted1 = encrypt_string(&key1, plaintext).unwrap();
        let encrypted2 = encrypt_string(&key2, plaintext).unwrap();

        // Should be different (different keys + random nonce)
        assert_ne!(encrypted1, encrypted2);

        // But each should decrypt correctly with its own key
        assert_eq!(plaintext, decrypt_string(&key1, &encrypted1).unwrap());
        assert_eq!(plaintext, decrypt_string(&key2, &encrypted2).unwrap());
    }

    #[test]
    fn test_wrong_key_fails_decryption() {
        let key1: [u8; 32] = [1u8; 32];
        let key2: [u8; 32] = [2u8; 32];
        let plaintext = "secret message";

        let encrypted = encrypt_string(&key1, plaintext).unwrap();

        // Trying to decrypt with wrong key should fail
        assert!(decrypt_string(&key2, &encrypted).is_err());
    }

    #[test]
    fn test_sensitive_column_detection() {
        let config = SensitiveColumnConfig::default();

        // Direct matches
        assert!(config.is_sensitive("jid"));
        assert!(config.is_sensitive("data"));
        assert!(config.is_sensitive("key"));

        // Suffix patterns
        assert!(config.is_sensitive("sender_jid"));
        assert!(config.is_sensitive("chat_jid"));
        assert!(config.is_sensitive("media_key"));
        assert!(config.is_sensitive("identity_key"));

        // Non-sensitive columns
        assert!(!config.is_sensitive("id"));
        assert!(!config.is_sensitive("created_at"));
        assert!(!config.is_sensitive("user_id"));
        assert!(!config.is_sensitive("status"));
    }
}
