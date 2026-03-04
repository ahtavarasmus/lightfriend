use crate::utils::encryption::{decrypt, encrypt};
use crate::{
    pg_models::{
        NewPgWebauthnChallenge, NewPgWebauthnCredential, PgWebauthnChallenge, PgWebauthnCredential,
    },
    pg_schema::{webauthn_challenges, webauthn_credentials},
    PgDbPool,
};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use std::time::{SystemTime, UNIX_EPOCH};

/// Parameters for creating a WebAuthn credential
pub struct CreateCredentialParams {
    pub user_id: i32,
    pub credential_id: String,
    pub public_key: String,
    pub device_name: String,
    pub counter: i32,
    pub transports: Option<String>,
    pub aaguid: Option<String>,
}

pub struct WebauthnRepository {
    pool: PgDbPool,
}

impl WebauthnRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    /// Create a new WebAuthn credential for a user
    pub fn create_credential(&self, params: CreateCredentialParams) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Encrypt the public key before storing
        let encrypted_public_key = encrypt(&params.public_key).map_err(|e| {
            DieselError::QueryBuilderError(Box::new(std::io::Error::other(format!(
                "Encryption error: {}",
                e
            ))))
        })?;

        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let new_credential = NewPgWebauthnCredential {
            user_id: params.user_id,
            credential_id: params.credential_id,
            encrypted_public_key,
            device_name: params.device_name,
            counter: params.counter,
            transports: params.transports,
            aaguid: params.aaguid,
            created_at: current_time,
            enabled: 1,
        };

        diesel::insert_into(webauthn_credentials::table)
            .values(&new_credential)
            .execute(&mut conn)?;

        Ok(())
    }

    /// Get all credentials for a user
    pub fn get_credentials_by_user(
        &self,
        user_id: i32,
    ) -> Result<Vec<PgWebauthnCredential>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        webauthn_credentials::table
            .filter(webauthn_credentials::user_id.eq(user_id))
            .filter(webauthn_credentials::enabled.eq(1))
            .select(PgWebauthnCredential::as_select())
            .load::<PgWebauthnCredential>(&mut conn)
    }

    /// Get decrypted public key for a credential
    pub fn get_decrypted_public_key(
        &self,
        credential: &PgWebauthnCredential,
    ) -> Result<String, DieselError> {
        decrypt(&credential.encrypted_public_key).map_err(|e| {
            DieselError::QueryBuilderError(Box::new(std::io::Error::other(format!(
                "Decryption error: {}",
                e
            ))))
        })
    }

    /// Update the signature counter for a credential
    pub fn update_counter(&self, credential_id: &str, new_counter: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        diesel::update(
            webauthn_credentials::table
                .filter(webauthn_credentials::credential_id.eq(credential_id)),
        )
        .set((
            webauthn_credentials::counter.eq(new_counter),
            webauthn_credentials::last_used_at.eq(current_time),
        ))
        .execute(&mut conn)?;

        Ok(())
    }

    /// Delete a credential
    pub fn delete_credential(
        &self,
        user_id: i32,
        credential_id: &str,
    ) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let rows_deleted = diesel::delete(
            webauthn_credentials::table
                .filter(webauthn_credentials::user_id.eq(user_id))
                .filter(webauthn_credentials::credential_id.eq(credential_id)),
        )
        .execute(&mut conn)?;

        Ok(rows_deleted > 0)
    }

    /// Rename a credential
    pub fn rename_credential(
        &self,
        user_id: i32,
        credential_id: &str,
        new_name: &str,
    ) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let rows_updated = diesel::update(
            webauthn_credentials::table
                .filter(webauthn_credentials::user_id.eq(user_id))
                .filter(webauthn_credentials::credential_id.eq(credential_id)),
        )
        .set(webauthn_credentials::device_name.eq(new_name))
        .execute(&mut conn)?;

        Ok(rows_updated > 0)
    }

    /// Check if a user has any passkeys
    pub fn has_passkeys(&self, user_id: i32) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let count: i64 = webauthn_credentials::table
            .filter(webauthn_credentials::user_id.eq(user_id))
            .filter(webauthn_credentials::enabled.eq(1))
            .count()
            .get_result(&mut conn)?;

        Ok(count > 0)
    }

    /// Get count of passkeys for a user
    pub fn get_passkey_count(&self, user_id: i32) -> Result<i64, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        webauthn_credentials::table
            .filter(webauthn_credentials::user_id.eq(user_id))
            .filter(webauthn_credentials::enabled.eq(1))
            .count()
            .get_result(&mut conn)
    }

    // ============ Challenge Management ============

    /// Create a new challenge for registration or authentication
    pub fn create_challenge(
        &self,
        user_id: i32,
        challenge: &str,
        challenge_type: &str,
        context: Option<String>,
        ttl_seconds: i64,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let expires_at = current_time + ttl_seconds as i32;

        // Delete any existing challenge of the same type for this user
        diesel::delete(
            webauthn_challenges::table
                .filter(webauthn_challenges::user_id.eq(user_id))
                .filter(webauthn_challenges::challenge_type.eq(challenge_type)),
        )
        .execute(&mut conn)?;

        let new_challenge = NewPgWebauthnChallenge {
            user_id,
            challenge: challenge.to_string(),
            challenge_type: challenge_type.to_string(),
            context,
            created_at: current_time,
            expires_at,
        };

        diesel::insert_into(webauthn_challenges::table)
            .values(&new_challenge)
            .execute(&mut conn)?;

        Ok(())
    }

    /// Get a valid (non-expired) challenge for a user
    pub fn get_valid_challenge(
        &self,
        user_id: i32,
        challenge_type: &str,
    ) -> Result<Option<PgWebauthnChallenge>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        webauthn_challenges::table
            .filter(webauthn_challenges::user_id.eq(user_id))
            .filter(webauthn_challenges::challenge_type.eq(challenge_type))
            .filter(webauthn_challenges::expires_at.gt(current_time))
            .select(PgWebauthnChallenge::as_select())
            .first::<PgWebauthnChallenge>(&mut conn)
            .optional()
    }

    /// Delete all challenges for a user of a specific type
    pub fn delete_challenges_by_type(
        &self,
        user_id: i32,
        challenge_type: &str,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::delete(
            webauthn_challenges::table
                .filter(webauthn_challenges::user_id.eq(user_id))
                .filter(webauthn_challenges::challenge_type.eq(challenge_type)),
        )
        .execute(&mut conn)?;

        Ok(())
    }
}
