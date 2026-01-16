use crate::utils::encryption::{decrypt, encrypt};
use crate::{
    models::user_models::{NewTotpBackupCode, NewTotpSecret, TotpBackupCode, TotpSecret},
    schema::{totp_backup_codes, totp_secrets},
    DbPool,
};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct TotpRepository {
    pool: DbPool,
}

impl TotpRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Create a new TOTP secret for a user (encrypted)
    /// The secret is stored but not enabled until verified
    pub fn create_secret(&self, user_id: i32, secret: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Encrypt the secret before storing
        let encrypted_secret = encrypt(secret).map_err(|e| {
            DieselError::QueryBuilderError(Box::new(std::io::Error::other(format!(
                "Encryption error: {}",
                e
            ))))
        })?;

        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let new_secret = NewTotpSecret {
            user_id,
            encrypted_secret,
            enabled: 0, // Not enabled until verified
            created_at: current_time,
        };

        // Delete any existing secret first (user might be re-setting up)
        diesel::delete(totp_secrets::table.filter(totp_secrets::user_id.eq(user_id)))
            .execute(&mut conn)?;

        diesel::insert_into(totp_secrets::table)
            .values(&new_secret)
            .execute(&mut conn)?;

        Ok(())
    }

    /// Get the decrypted TOTP secret for a user
    pub fn get_secret(&self, user_id: i32) -> Result<Option<String>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let secret_opt = totp_secrets::table
            .filter(totp_secrets::user_id.eq(user_id))
            .select(TotpSecret::as_select())
            .first::<TotpSecret>(&mut conn)
            .optional()?;

        match secret_opt {
            Some(secret) => {
                let decrypted = decrypt(&secret.encrypted_secret).map_err(|e| {
                    DieselError::QueryBuilderError(Box::new(std::io::Error::other(format!(
                        "Decryption error: {}",
                        e
                    ))))
                })?;
                Ok(Some(decrypted))
            }
            None => Ok(None),
        }
    }

    /// Check if TOTP is enabled for a user
    pub fn is_totp_enabled(&self, user_id: i32) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let enabled_opt: Option<i32> = totp_secrets::table
            .filter(totp_secrets::user_id.eq(user_id))
            .select(totp_secrets::enabled)
            .first(&mut conn)
            .optional()?;

        Ok(enabled_opt.unwrap_or(0) == 1)
    }

    /// Enable TOTP for a user (called after successful verification)
    pub fn enable_totp(&self, user_id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::update(totp_secrets::table.filter(totp_secrets::user_id.eq(user_id)))
            .set(totp_secrets::enabled.eq(1))
            .execute(&mut conn)?;

        Ok(())
    }

    /// Disable TOTP for a user
    pub fn disable_totp(&self, user_id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Delete the secret
        diesel::delete(totp_secrets::table.filter(totp_secrets::user_id.eq(user_id)))
            .execute(&mut conn)?;

        // Delete all backup codes
        diesel::delete(totp_backup_codes::table.filter(totp_backup_codes::user_id.eq(user_id)))
            .execute(&mut conn)?;

        Ok(())
    }

    /// Create backup codes for a user (hashed with bcrypt)
    /// Returns the plain text codes to show to the user once
    pub fn create_backup_codes(&self, user_id: i32) -> Result<Vec<String>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Delete any existing backup codes
        diesel::delete(totp_backup_codes::table.filter(totp_backup_codes::user_id.eq(user_id)))
            .execute(&mut conn)?;

        // Generate 10 backup codes
        let mut codes = Vec::new();
        let mut rng = rand::thread_rng();

        for _ in 0..10 {
            // Generate 8-character alphanumeric code
            let code: String = (0..8)
                .map(|_| {
                    let idx = rng.gen_range(0..36);
                    if idx < 10 {
                        (b'0' + idx) as char
                    } else {
                        (b'A' + idx - 10) as char
                    }
                })
                .collect();

            // Hash the code with bcrypt
            let code_hash = bcrypt::hash(&code, bcrypt::DEFAULT_COST).map_err(|e| {
                DieselError::QueryBuilderError(Box::new(std::io::Error::other(format!(
                    "Bcrypt error: {}",
                    e
                ))))
            })?;

            let new_code = NewTotpBackupCode {
                user_id,
                code_hash,
                used: 0,
            };

            diesel::insert_into(totp_backup_codes::table)
                .values(&new_code)
                .execute(&mut conn)?;

            codes.push(code);
        }

        Ok(codes)
    }

    /// Verify a backup code and mark it as used if valid
    /// Returns true if the code was valid and unused
    pub fn verify_backup_code(&self, user_id: i32, code: &str) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Get all unused backup codes for the user
        let backup_codes = totp_backup_codes::table
            .filter(totp_backup_codes::user_id.eq(user_id))
            .filter(totp_backup_codes::used.eq(0))
            .select(TotpBackupCode::as_select())
            .load::<TotpBackupCode>(&mut conn)?;

        // Check each code
        for backup_code in backup_codes {
            if bcrypt::verify(code, &backup_code.code_hash).unwrap_or(false) {
                // Mark as used
                diesel::update(
                    totp_backup_codes::table.filter(totp_backup_codes::id.eq(backup_code.id)),
                )
                .set(totp_backup_codes::used.eq(1))
                .execute(&mut conn)?;

                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Get the count of remaining (unused) backup codes
    pub fn get_remaining_backup_codes(&self, user_id: i32) -> Result<i64, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let count: i64 = totp_backup_codes::table
            .filter(totp_backup_codes::user_id.eq(user_id))
            .filter(totp_backup_codes::used.eq(0))
            .count()
            .get_result(&mut conn)?;

        Ok(count)
    }
}
