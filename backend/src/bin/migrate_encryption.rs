use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use dotenvy::dotenv;
use magic_crypt::MagicCryptTrait;
use std::env;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm,
    Nonce,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::Rng;

/// Encrypts a string using AES-GCM encryption
fn encrypt(value: &str) -> Result<String, String> {
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
        .encrypt(nonce, value.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;
    
    let mut combined = nonce_bytes.to_vec();
    combined.extend(ciphertext);
    
    Ok(BASE64.encode(combined))
}

/// Copied over from utils/encryption.rs
pub fn decrypt(encrypted: &str) -> Result<String, String> {
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

#[derive(QueryableByName)]
struct UserCreds {
    #[diesel(sql_type = diesel::sql_types::Integer)]
    id: i32,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    encrypted_matrix_password: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    encrypted_matrix_access_token: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    matrix_device_id: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    encrypted_matrix_secret_storage_recovery_key: Option<String>,
}

#[derive(QueryableByName)]
struct GoogleTokens {
    #[diesel(sql_type = diesel::sql_types::Integer)]
    user_id: i32,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    encrypted_access_token: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    encrypted_refresh_token: Option<String>,
}

#[derive(QueryableByName)]
struct ImapPassword {
    #[diesel(sql_type = diesel::sql_types::Integer)]
    user_id: i32,
    #[diesel(sql_type = diesel::sql_types::Text)]
    encrypted_password: String,
}

fn run_migration(conn: &mut SqliteConnection) -> Result<(), Box<dyn std::error::Error>> {
    let encryption_key = env::var("ENCRYPTION_KEY").expect("ENCRYPTION_KEY must be set");
    let mc = magic_crypt::new_magic_crypt!(&encryption_key, 256);

    // Migrate Matrix credentials
    let users = diesel::sql_query("SELECT id, encrypted_matrix_password, encrypted_matrix_access_token, matrix_device_id, encrypted_matrix_secret_storage_recovery_key FROM users WHERE encrypted_matrix_password IS NOT NULL OR encrypted_matrix_access_token IS NOT NULL")
        .load::<UserCreds>(conn)?;

    for user in users {
        if let Some(pwd) = user.encrypted_matrix_password {
            let decrypted = mc.decrypt_base64_to_string(&pwd)?;
            let encrypted = encrypt(&decrypted)?;
            diesel::sql_query("UPDATE users SET encrypted_matrix_password = ? WHERE id = ?")
                .bind::<diesel::sql_types::Text, _>(&encrypted)
                .bind::<diesel::sql_types::Integer, _>(user.id)
                .execute(conn)?;
        }

        if let Some(token) = user.encrypted_matrix_access_token {
            let decrypted = mc.decrypt_base64_to_string(&token)?;
            let encrypted = encrypt(&decrypted)?;
            diesel::sql_query("UPDATE users SET encrypted_matrix_access_token = ? WHERE id = ?")
                .bind::<diesel::sql_types::Text, _>(&encrypted)
                .bind::<diesel::sql_types::Integer, _>(user.id)
                .execute(conn)?;
        }

        if let Some(device) = user.matrix_device_id {
            let decrypted = mc.decrypt_base64_to_string(&device)?;
            let encrypted = encrypt(&decrypted)?;
            diesel::sql_query("UPDATE users SET matrix_device_id = ? WHERE id = ?")
                .bind::<diesel::sql_types::Text, _>(&encrypted)
                .bind::<diesel::sql_types::Integer, _>(user.id)
                .execute(conn)?;
        }

        if let Some(key) = user.encrypted_matrix_secret_storage_recovery_key {
            let decrypted = mc.decrypt_base64_to_string(&key)?;
            let encrypted = encrypt(&decrypted)?;
            diesel::sql_query("UPDATE users SET encrypted_matrix_secret_storage_recovery_key = ? WHERE id = ?")
                .bind::<diesel::sql_types::Text, _>(&encrypted)
                .bind::<diesel::sql_types::Integer, _>(user.id)
                .execute(conn)?;
        }
    }

    // Migrate Google Calendar tokens
    let calendar_tokens = diesel::sql_query("SELECT user_id, encrypted_access_token, encrypted_refresh_token FROM google_calendar WHERE encrypted_access_token IS NOT NULL OR encrypted_refresh_token IS NOT NULL")
        .load::<GoogleTokens>(conn)?;

    for token in calendar_tokens {
        if let Some(access_token) = token.encrypted_access_token {
            let decrypted = mc.decrypt_base64_to_string(&access_token)?;
            let encrypted = encrypt(&decrypted)?;
            diesel::sql_query("UPDATE google_calendar SET encrypted_access_token = ? WHERE user_id = ?")
                .bind::<diesel::sql_types::Text, _>(&encrypted)
                .bind::<diesel::sql_types::Integer, _>(token.user_id)
                .execute(conn)?;
        }

        if let Some(refresh_token) = token.encrypted_refresh_token {
            let decrypted = mc.decrypt_base64_to_string(&refresh_token)?;
            let encrypted = encrypt(&decrypted)?;
            diesel::sql_query("UPDATE google_calendar SET encrypted_refresh_token = ? WHERE user_id = ?")
                .bind::<diesel::sql_types::Text, _>(&encrypted)
                .bind::<diesel::sql_types::Integer, _>(token.user_id)
                .execute(conn)?;
        }
    }

    // Migrate Gmail tokens
    let gmail_tokens = diesel::sql_query("SELECT user_id, encrypted_access_token, encrypted_refresh_token FROM gmail WHERE encrypted_access_token IS NOT NULL OR encrypted_refresh_token IS NOT NULL")
        .load::<GoogleTokens>(conn)?;

    for token in gmail_tokens {
        if let Some(access_token) = token.encrypted_access_token {
            let decrypted = mc.decrypt_base64_to_string(&access_token)?;
            let encrypted = encrypt(&decrypted)?;
            diesel::sql_query("UPDATE gmail SET encrypted_access_token = ? WHERE user_id = ?")
                .bind::<diesel::sql_types::Text, _>(&encrypted)
                .bind::<diesel::sql_types::Integer, _>(token.user_id)
                .execute(conn)?;
        }

        if let Some(refresh_token) = token.encrypted_refresh_token {
            let decrypted = mc.decrypt_base64_to_string(&refresh_token)?;
            let encrypted = encrypt(&decrypted)?;
            diesel::sql_query("UPDATE gmail SET encrypted_refresh_token = ? WHERE user_id = ?")
                .bind::<diesel::sql_types::Text, _>(&encrypted)
                .bind::<diesel::sql_types::Integer, _>(token.user_id)
                .execute(conn)?;
        }
    }

    // Migrate Google Tasks tokens
    let tasks_tokens = diesel::sql_query("SELECT user_id, encrypted_access_token, encrypted_refresh_token FROM google_tasks WHERE encrypted_access_token IS NOT NULL OR encrypted_refresh_token IS NOT NULL")
        .load::<GoogleTokens>(conn)?;

    for token in tasks_tokens {
        if let Some(access_token) = token.encrypted_access_token {
            let decrypted = mc.decrypt_base64_to_string(&access_token)?;
            let encrypted = encrypt(&decrypted)?;
            diesel::sql_query("UPDATE google_tasks SET encrypted_access_token = ? WHERE user_id = ?")
                .bind::<diesel::sql_types::Text, _>(&encrypted)
                .bind::<diesel::sql_types::Integer, _>(token.user_id)
                .execute(conn)?;
        }

        if let Some(refresh_token) = token.encrypted_refresh_token {
            let decrypted = mc.decrypt_base64_to_string(&refresh_token)?;
            let encrypted = encrypt(&decrypted)?;
            diesel::sql_query("UPDATE google_tasks SET encrypted_refresh_token = ? WHERE user_id = ?")
                .bind::<diesel::sql_types::Text, _>(&encrypted)
                .bind::<diesel::sql_types::Integer, _>(token.user_id)
                .execute(conn)?;
        }
    }

    // Migrate IMAP passwords
    let imap_passwords = diesel::sql_query("SELECT user_id, encrypted_password FROM imap_connection WHERE encrypted_password IS NOT NULL")
        .load::<ImapPassword>(conn)?;

    for password in imap_passwords {
        let decrypted = mc.decrypt_base64_to_string(&password.encrypted_password)?;
        let encrypted = encrypt(&decrypted)?;
        diesel::sql_query("UPDATE imap_connection SET encrypted_password = ? WHERE user_id = ?")
            .bind::<diesel::sql_types::Text, _>(&encrypted)
            .bind::<diesel::sql_types::Integer, _>(password.user_id)
            .execute(conn)?;
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let manager = ConnectionManager::<SqliteConnection>::new("database.db");
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool");

    println!("Starting encryption migration...");
    
    let mut conn = pool.get()?;
    run_migration(&mut conn)?;
    
    println!("Encryption migration completed successfully!");
    Ok(())
} 