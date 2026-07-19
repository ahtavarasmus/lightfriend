use crate::{
    models::light_tool_models::{LightToolPushRegistration, NewLightToolPushRegistration},
    pg_schema::light_tool_push_registrations,
    utils::encryption::{decrypt, encrypt, EncryptionError},
    PgDbPool,
};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LightToolPushRegistrationRecord {
    pub device_id: i32,
    pub endpoint: String,
    pub endpoint_hash: String,
    pub registered_at: i32,
    pub updated_at: i32,
}

#[derive(Debug, Error)]
pub enum LightToolPushRepositoryError {
    #[error(transparent)]
    Database(#[from] DieselError),
    #[error(transparent)]
    Encryption(#[from] EncryptionError),
}

pub struct LightToolPushRepository {
    pool: PgDbPool,
}

impl LightToolPushRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    pub fn upsert(
        &self,
        device_id: i32,
        endpoint: &str,
        now: i32,
    ) -> Result<LightToolPushRegistrationRecord, LightToolPushRepositoryError> {
        let registration = NewLightToolPushRegistration {
            device_id,
            encrypted_endpoint: encrypt(endpoint)?,
            endpoint_hash: endpoint_hash(endpoint),
            registered_at: now,
            updated_at: now,
        };
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let stored = diesel::insert_into(light_tool_push_registrations::table)
            .values(&registration)
            .on_conflict(light_tool_push_registrations::device_id)
            .do_update()
            .set((
                light_tool_push_registrations::encrypted_endpoint
                    .eq(&registration.encrypted_endpoint),
                light_tool_push_registrations::endpoint_hash.eq(&registration.endpoint_hash),
                light_tool_push_registrations::registered_at.eq(registration.registered_at),
                light_tool_push_registrations::updated_at.eq(registration.updated_at),
            ))
            .get_result::<LightToolPushRegistration>(&mut conn)?;
        decrypt_registration(stored).map_err(Into::into)
    }

    pub fn find_for_device(
        &self,
        device_id: i32,
    ) -> Result<Option<LightToolPushRegistrationRecord>, LightToolPushRepositoryError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let registration = light_tool_push_registrations::table
            .find(device_id)
            .select(LightToolPushRegistration::as_select())
            .first::<LightToolPushRegistration>(&mut conn)
            .optional()?;
        registration
            .map(decrypt_registration)
            .transpose()
            .map_err(Into::into)
    }

    pub fn delete_for_device(&self, device_id: i32) -> Result<bool, LightToolPushRepositoryError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        Ok(
            diesel::delete(light_tool_push_registrations::table.find(device_id))
                .execute(&mut conn)?
                > 0,
        )
    }

    pub fn delete_for_device_if_endpoint_hash(
        &self,
        device_id: i32,
        expected_endpoint_hash: &str,
    ) -> Result<bool, LightToolPushRepositoryError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        Ok(diesel::delete(
            light_tool_push_registrations::table
                .filter(light_tool_push_registrations::device_id.eq(device_id))
                .filter(light_tool_push_registrations::endpoint_hash.eq(expected_endpoint_hash)),
        )
        .execute(&mut conn)?
            > 0)
    }
}

fn endpoint_hash(endpoint: &str) -> String {
    hex::encode(Sha256::digest(endpoint.as_bytes()))
}

fn decrypt_registration(
    registration: LightToolPushRegistration,
) -> Result<LightToolPushRegistrationRecord, EncryptionError> {
    Ok(LightToolPushRegistrationRecord {
        device_id: registration.device_id,
        endpoint: decrypt(&registration.encrypted_endpoint)?,
        endpoint_hash: registration.endpoint_hash,
        registered_at: registration.registered_at,
        updated_at: registration.updated_at,
    })
}
