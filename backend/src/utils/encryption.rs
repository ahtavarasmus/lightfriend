use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm,
    Nonce,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::Rng;

pub fn encrypt_token(token: &str) -> Result<String, String> {
    let encryption_key = std::env::var("ENCRYPTION_KEY")
        .expect("ENCRYPTION_KEY must be set");
    
    let key = BASE64.decode(encryption_key)
        .map_err(|e| format!("Failed to decode key: {}", e))?;
    
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("Failed to create cipher: {}", e))?;
    
    let mut rng = rand::thread_rng();
    let mut nonce_bytes = [0u8; 12];
    rng.fill(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    let ciphertext = cipher
        .encrypt(nonce, token.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;
    
    let mut combined = nonce_bytes.to_vec();
    combined.extend(ciphertext);
    
    Ok(BASE64.encode(combined))
}

pub fn decrypt_token(encrypted: &str) -> Result<String, String> {
    let encryption_key = std::env::var("ENCRYPTION_KEY")
        .expect("ENCRYPTION_KEY must be set");
    
    let key = BASE64.decode(encryption_key)
        .map_err(|e| format!("Failed to decode key: {}", e))?;
    
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("Failed to create cipher: {}", e))?;
    
    let encrypted_data = BASE64.decode(encrypted)
        .map_err(|e| format!("Failed to decode encrypted data: {}", e))?;
    
    if encrypted_data.len() < 12 {
        return Err("Invalid encrypted data".to_string());
    }
    
    let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption failed: {}", e))?;
    
    String::from_utf8(plaintext)
        .map_err(|e| format!("Invalid UTF-8: {}", e))
}

