use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm,
    Nonce,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::Rng;
use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EncryptionError {
    #[error("Failed to decode key: {0}")]
    KeyDecodeError(String),
    #[error("Failed to create cipher: {0}")]
    CipherError(String),
    #[error("Encryption failed: {0}")]
    EncryptionError(String),
    #[error("Decryption failed: {0}")]
    DecryptionError(String),
    #[error("Invalid UTF-8: {0}")]
    Utf8Error(String),
    #[error("Invalid encrypted data")]
    InvalidData,
    #[error("Environment error: {0}")]
    EnvError(#[from] std::env::VarError),
}

/// Encrypts a string using AES-GCM encryption
/// 
/// # Arguments
/// * `value` - The string to encrypt
/// 
/// # Returns
/// The encrypted string in base64 format
pub fn encrypt(value: &str) -> Result<String, EncryptionError> {
    let encryption_key = std::env::var("ENCRYPTION_KEY")?;
    
    let key = BASE64.decode(encryption_key)
        .map_err(|e| EncryptionError::KeyDecodeError(e.to_string()))?;
    
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| EncryptionError::CipherError(e.to_string()))?;
    
    let mut rng = rand::thread_rng();
    let mut nonce_bytes = [0u8; 12];
    rng.fill(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    let ciphertext = cipher
        .encrypt(nonce, value.as_bytes())
        .map_err(|e| EncryptionError::EncryptionError(e.to_string()))?;
    
    let mut combined = nonce_bytes.to_vec();
    combined.extend(ciphertext);
    
    Ok(BASE64.encode(combined))
}

/// Decrypts a string that was encrypted using AES-GCM
/// 
/// # Arguments
/// * `encrypted` - The encrypted string in base64 format
/// 
/// # Returns
/// The decrypted string
pub fn decrypt(encrypted: &str) -> Result<String, EncryptionError> {
    let encryption_key = std::env::var("ENCRYPTION_KEY")?;
    
    let key = BASE64.decode(encryption_key)
        .map_err(|e| EncryptionError::KeyDecodeError(e.to_string()))?;
    
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| EncryptionError::CipherError(e.to_string()))?;
    
    let encrypted_data = BASE64.decode(encrypted)
        .map_err(|e| EncryptionError::KeyDecodeError(e.to_string()))?;
    
    if encrypted_data.len() < 12 {
        return Err(EncryptionError::InvalidData);
    }
    
    let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| EncryptionError::DecryptionError(e.to_string()))?;
    
    String::from_utf8(plaintext)
        .map_err(|e| EncryptionError::Utf8Error(e.to_string()))
}

// For backward compatibility with existing code
pub fn encrypt_token(token: &str) -> Result<String, EncryptionError> {
    encrypt(token)
}

pub fn decrypt_token(encrypted: &str) -> Result<String, EncryptionError> {
    decrypt(encrypted)
}

