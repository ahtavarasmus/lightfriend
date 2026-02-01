//! Matrix store encryption service for encrypted backups.
//!
//! This module provides full-file encryption for Matrix SQLite stores.
//! Each user's matrix-sdk-crypto.sqlite3 file is encrypted as a whole
//! using their session key.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::Rng;
use std::path::Path;
use thiserror::Error;
use tokio::fs;
use tokio::io::AsyncWriteExt;

#[derive(Error, Debug)]
pub enum MatrixStoreEncryptionError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),
    #[error("Store not found: {0}")]
    StoreNotFound(String),
    #[error("Invalid encrypted format")]
    InvalidFormat,
}

/// Magic bytes to identify our encrypted format
const ENCRYPTED_MAGIC: &[u8; 8] = b"LFMATRIX";

/// Version of the encryption format
const FORMAT_VERSION: u8 = 1;

/// Encrypt a Matrix store file using the user's session key
///
/// The encrypted file format is:
/// - 8 bytes: magic "LFMATRIX"
/// - 1 byte: format version
/// - 12 bytes: nonce
/// - rest: AES-GCM ciphertext
pub async fn encrypt_matrix_store(
    user_id: i32,
    session_key: &[u8; 32],
    matrix_stores_dir: &str,
    encrypted_output_dir: &str,
) -> Result<(), MatrixStoreEncryptionError> {
    let store_path = format!(
        "{}/{}/matrix-sdk-crypto.sqlite3",
        matrix_stores_dir, user_id
    );
    let encrypted_path = format!("{}/{}.sqlite.enc", encrypted_output_dir, user_id);

    // Check if store exists
    if !Path::new(&store_path).exists() {
        tracing::debug!(
            "Matrix store not found for user {}: {}",
            user_id,
            store_path
        );
        return Err(MatrixStoreEncryptionError::StoreNotFound(store_path));
    }

    // Generate random nonce BEFORE any async operations to avoid Send issues
    let nonce_bytes: [u8; 12] = {
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; 12];
        rng.fill(&mut bytes);
        bytes
    };
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Read the plaintext store
    let plaintext = fs::read(&store_path).await?;

    // Create cipher
    let cipher = Aes256Gcm::new_from_slice(session_key)
        .map_err(|e| MatrixStoreEncryptionError::EncryptionFailed(e.to_string()))?;

    // Encrypt
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_ref())
        .map_err(|e| MatrixStoreEncryptionError::EncryptionFailed(e.to_string()))?;

    // Build output: magic + version + nonce + ciphertext
    let mut encrypted = Vec::with_capacity(8 + 1 + 12 + ciphertext.len());
    encrypted.extend_from_slice(ENCRYPTED_MAGIC);
    encrypted.push(FORMAT_VERSION);
    encrypted.extend_from_slice(&nonce_bytes);
    encrypted.extend(ciphertext);

    // Ensure output directory exists
    let output_dir = Path::new(encrypted_output_dir);
    if !output_dir.exists() {
        fs::create_dir_all(output_dir).await?;
    }

    // Write encrypted file atomically (write to temp, then rename)
    let temp_path = format!("{}.tmp", encrypted_path);
    let mut file = fs::File::create(&temp_path).await?;
    file.write_all(&encrypted).await?;
    file.sync_all().await?;
    fs::rename(&temp_path, &encrypted_path).await?;

    tracing::info!(
        "Encrypted Matrix store for user {} ({} bytes -> {} bytes)",
        user_id,
        plaintext.len(),
        encrypted.len()
    );

    Ok(())
}

/// Decrypt a Matrix store file using the user's session key
pub async fn decrypt_matrix_store(
    user_id: i32,
    session_key: &[u8; 32],
    encrypted_input_dir: &str,
    matrix_stores_dir: &str,
) -> Result<(), MatrixStoreEncryptionError> {
    let encrypted_path = format!("{}/{}.sqlite.enc", encrypted_input_dir, user_id);
    let store_dir = format!("{}/{}", matrix_stores_dir, user_id);
    let store_path = format!("{}/matrix-sdk-crypto.sqlite3", store_dir);

    // Check if encrypted file exists
    if !Path::new(&encrypted_path).exists() {
        return Err(MatrixStoreEncryptionError::StoreNotFound(encrypted_path));
    }

    // Read encrypted file
    let encrypted = fs::read(&encrypted_path).await?;

    // Validate format
    if encrypted.len() < 8 + 1 + 12 {
        return Err(MatrixStoreEncryptionError::InvalidFormat);
    }

    // Check magic
    if &encrypted[0..8] != ENCRYPTED_MAGIC {
        return Err(MatrixStoreEncryptionError::InvalidFormat);
    }

    // Check version
    let version = encrypted[8];
    if version != FORMAT_VERSION {
        return Err(MatrixStoreEncryptionError::DecryptionFailed(format!(
            "Unsupported format version: {}",
            version
        )));
    }

    // Extract nonce and ciphertext
    let nonce_bytes = &encrypted[9..21];
    let ciphertext = &encrypted[21..];
    let nonce = Nonce::from_slice(nonce_bytes);

    // Create cipher
    let cipher = Aes256Gcm::new_from_slice(session_key)
        .map_err(|e| MatrixStoreEncryptionError::DecryptionFailed(e.to_string()))?;

    // Decrypt
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| MatrixStoreEncryptionError::DecryptionFailed(e.to_string()))?;

    // Ensure output directory exists
    if !Path::new(&store_dir).exists() {
        fs::create_dir_all(&store_dir).await?;
    }

    // Write decrypted file atomically
    let temp_path = format!("{}.tmp", store_path);
    let mut file = fs::File::create(&temp_path).await?;
    file.write_all(&plaintext).await?;
    file.sync_all().await?;
    fs::rename(&temp_path, &store_path).await?;

    tracing::info!(
        "Decrypted Matrix store for user {} ({} bytes)",
        user_id,
        plaintext.len()
    );

    Ok(())
}

/// Check if an encrypted backup exists for a user
pub async fn backup_exists(user_id: i32, encrypted_output_dir: &str) -> bool {
    let encrypted_path = format!("{}/{}.sqlite.enc", encrypted_output_dir, user_id);
    Path::new(&encrypted_path).exists()
}

/// Get the size of the encrypted backup (if it exists)
pub async fn get_backup_size(
    user_id: i32,
    encrypted_output_dir: &str,
) -> Result<Option<u64>, std::io::Error> {
    let encrypted_path = format!("{}/{}.sqlite.enc", encrypted_output_dir, user_id);
    match fs::metadata(&encrypted_path).await {
        Ok(meta) => Ok(Some(meta.len())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_encrypt_decrypt_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let stores_dir = temp_dir.path().join("stores");
        let encrypted_dir = temp_dir.path().join("encrypted");

        // Create user store directory and file
        let user_store_dir = stores_dir.join("123");
        fs::create_dir_all(&user_store_dir).await.unwrap();

        let original_content = b"SQLite format 3\x00test database content here";
        let store_file = user_store_dir.join("matrix-sdk-crypto.sqlite3");
        fs::write(&store_file, original_content).await.unwrap();

        let session_key: [u8; 32] = [42u8; 32];

        // Encrypt
        encrypt_matrix_store(
            123,
            &session_key,
            stores_dir.to_str().unwrap(),
            encrypted_dir.to_str().unwrap(),
        )
        .await
        .unwrap();

        // Verify encrypted file exists
        assert!(backup_exists(123, encrypted_dir.to_str().unwrap()).await);

        // Delete original
        fs::remove_file(&store_file).await.unwrap();

        // Decrypt
        decrypt_matrix_store(
            123,
            &session_key,
            encrypted_dir.to_str().unwrap(),
            stores_dir.to_str().unwrap(),
        )
        .await
        .unwrap();

        // Verify content matches
        let restored = fs::read(&store_file).await.unwrap();
        assert_eq!(original_content.to_vec(), restored);
    }

    #[tokio::test]
    async fn test_wrong_key_fails() {
        let temp_dir = TempDir::new().unwrap();
        let stores_dir = temp_dir.path().join("stores");
        let encrypted_dir = temp_dir.path().join("encrypted");

        // Create user store
        let user_store_dir = stores_dir.join("456");
        fs::create_dir_all(&user_store_dir).await.unwrap();
        let store_file = user_store_dir.join("matrix-sdk-crypto.sqlite3");
        fs::write(&store_file, b"secret data").await.unwrap();

        let key1: [u8; 32] = [1u8; 32];
        let key2: [u8; 32] = [2u8; 32];

        // Encrypt with key1
        encrypt_matrix_store(
            456,
            &key1,
            stores_dir.to_str().unwrap(),
            encrypted_dir.to_str().unwrap(),
        )
        .await
        .unwrap();

        // Delete original
        fs::remove_file(&store_file).await.unwrap();

        // Try to decrypt with key2 - should fail
        let result = decrypt_matrix_store(
            456,
            &key2,
            encrypted_dir.to_str().unwrap(),
            stores_dir.to_str().unwrap(),
        )
        .await;

        assert!(result.is_err());
    }
}
