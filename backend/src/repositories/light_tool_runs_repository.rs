use crate::{
    models::light_tool_models::{LightToolDevice, LightToolRun, NewLightToolRun},
    pg_schema::{light_tool_devices, light_tool_runs},
    utils::encryption::{decrypt, encrypt, EncryptionError},
    PgDbPool,
};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LightToolRunRecord {
    pub id: String,
    pub device_id: i32,
    pub account_user_id: Option<i32>,
    pub client_message_id: String,
    pub user_message: String,
    pub activity_text: Option<String>,
    pub assistant_message: Option<String>,
    pub error_message: Option<String>,
    pub status: String,
    pub created_at: i32,
    pub updated_at: i32,
    pub completed_at: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnonymousTrialRunCreation {
    Created {
        run: LightToolRunRecord,
        messages_remaining: i32,
    },
    Existing {
        run: LightToolRunRecord,
        messages_remaining: i32,
    },
    TrialUnavailable,
    IdempotencyConflict,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccountRunCreation {
    Created(LightToolRunRecord),
    Existing(LightToolRunRecord),
    AccountUnavailable,
    IdempotencyConflict,
}

#[derive(Debug, Error)]
pub enum LightToolRunsRepositoryError {
    #[error(transparent)]
    Database(#[from] DieselError),
    #[error(transparent)]
    Encryption(#[from] EncryptionError),
}

pub struct LightToolRunsRepository {
    pool: PgDbPool,
}

impl LightToolRunsRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    /// Creates one anonymous-trial run and claims its quota in the same
    /// transaction. Locking the device serializes retries with the same
    /// client message id, so a replay returns the original run for free.
    pub fn create_anonymous_trial_run(
        &self,
        device_id: i32,
        client_message_id: &str,
        user_message: &str,
        now: i32,
        message_limit: i32,
    ) -> Result<AnonymousTrialRunCreation, LightToolRunsRepositoryError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        conn.transaction::<AnonymousTrialRunCreation, LightToolRunsRepositoryError, _>(|conn| {
            let device = light_tool_devices::table
                .filter(light_tool_devices::id.eq(device_id))
                .filter(light_tool_devices::revoked_at.is_null())
                .for_update()
                .select(LightToolDevice::as_select())
                .first::<LightToolDevice>(conn)
                .optional()?;

            let Some(device) = device else {
                return Ok(AnonymousTrialRunCreation::TrialUnavailable);
            };

            if device.user_id.is_some() {
                return Ok(AnonymousTrialRunCreation::TrialUnavailable);
            }

            let existing = light_tool_runs::table
                .filter(light_tool_runs::device_id.eq(device_id))
                .filter(light_tool_runs::client_message_id.eq(client_message_id))
                .select(LightToolRun::as_select())
                .first::<LightToolRun>(conn)
                .optional()?;

            if let Some(existing) = existing {
                if existing.account_user_id.is_some() {
                    return Ok(AnonymousTrialRunCreation::IdempotencyConflict);
                }
                return Ok(AnonymousTrialRunCreation::Existing {
                    run: decrypt_run(existing)?,
                    messages_remaining: remaining_messages(&device, message_limit),
                });
            }

            let claimed_device = diesel::update(
                light_tool_devices::table
                    .filter(light_tool_devices::id.eq(device_id))
                    .filter(light_tool_devices::user_id.is_null())
                    .filter(light_tool_devices::revoked_at.is_null())
                    .filter(light_tool_devices::trial_expires_at.gt(now))
                    .filter(light_tool_devices::trial_messages_used.lt(message_limit)),
            )
            .set((
                light_tool_devices::trial_messages_used
                    .eq(light_tool_devices::trial_messages_used + 1),
                light_tool_devices::updated_at.eq(now),
            ))
            .get_result::<LightToolDevice>(conn)
            .optional()?;

            let Some(claimed_device) = claimed_device else {
                return Ok(AnonymousTrialRunCreation::TrialUnavailable);
            };

            let new_run = NewLightToolRun {
                id: Uuid::new_v4().to_string(),
                device_id,
                account_user_id: None,
                client_message_id: client_message_id.to_string(),
                encrypted_user_message: encrypt(user_message)?,
                created_at: now,
                updated_at: now,
            };
            let run = diesel::insert_into(light_tool_runs::table)
                .values(&new_run)
                .get_result::<LightToolRun>(conn)?;

            Ok(AnonymousTrialRunCreation::Created {
                run: decrypt_run(run)?,
                messages_remaining: remaining_messages(&claimed_device, message_limit),
            })
        })
    }

    /// Creates a run for the account currently linked to this device. The
    /// account id is copied onto the run so later pairing changes cannot alter
    /// which authorization boundary the queued work executes under.
    pub fn create_account_run(
        &self,
        device_id: i32,
        account_user_id: i32,
        client_message_id: &str,
        user_message: &str,
        now: i32,
    ) -> Result<AccountRunCreation, LightToolRunsRepositoryError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        conn.transaction::<AccountRunCreation, LightToolRunsRepositoryError, _>(|conn| {
            let device = light_tool_devices::table
                .filter(light_tool_devices::id.eq(device_id))
                .filter(light_tool_devices::revoked_at.is_null())
                .for_update()
                .select(LightToolDevice::as_select())
                .first::<LightToolDevice>(conn)
                .optional()?;

            let Some(device) = device else {
                return Ok(AccountRunCreation::AccountUnavailable);
            };
            if device.user_id != Some(account_user_id) {
                return Ok(AccountRunCreation::AccountUnavailable);
            }

            let existing = light_tool_runs::table
                .filter(light_tool_runs::device_id.eq(device_id))
                .filter(light_tool_runs::client_message_id.eq(client_message_id))
                .select(LightToolRun::as_select())
                .first::<LightToolRun>(conn)
                .optional()?;

            if let Some(existing) = existing {
                if existing.account_user_id != Some(account_user_id) {
                    return Ok(AccountRunCreation::IdempotencyConflict);
                }
                return Ok(AccountRunCreation::Existing(decrypt_run(existing)?));
            }

            let new_run = NewLightToolRun {
                id: Uuid::new_v4().to_string(),
                device_id,
                account_user_id: Some(account_user_id),
                client_message_id: client_message_id.to_string(),
                encrypted_user_message: encrypt(user_message)?,
                created_at: now,
                updated_at: now,
            };
            let run = diesel::insert_into(light_tool_runs::table)
                .values(&new_run)
                .get_result::<LightToolRun>(conn)?;

            Ok(AccountRunCreation::Created(decrypt_run(run)?))
        })
    }

    pub fn find_by_id_for_device(
        &self,
        run_id: &str,
        device_id: i32,
    ) -> Result<Option<LightToolRunRecord>, LightToolRunsRepositoryError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let run = light_tool_runs::table
            .filter(light_tool_runs::id.eq(run_id))
            .filter(light_tool_runs::device_id.eq(device_id))
            .select(LightToolRun::as_select())
            .first::<LightToolRun>(&mut conn)
            .optional()?;
        Ok(run.map(decrypt_run).transpose()?)
    }

    pub fn find_recent_completed_for_principal(
        &self,
        device_id: i32,
        account_user_id: Option<i32>,
        before_created_at: i32,
        limit: i64,
    ) -> Result<Vec<LightToolRunRecord>, LightToolRunsRepositoryError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let query = light_tool_runs::table
            .filter(light_tool_runs::device_id.eq(device_id))
            .filter(light_tool_runs::status.eq("completed"))
            .filter(light_tool_runs::created_at.le(before_created_at))
            .filter(light_tool_runs::encrypted_assistant_message.is_not_null())
            .into_boxed();
        let query = match account_user_id {
            Some(user_id) => query.filter(light_tool_runs::account_user_id.eq(user_id)),
            None => query.filter(light_tool_runs::account_user_id.is_null()),
        };
        let mut runs = query
            .order((
                light_tool_runs::created_at.desc(),
                light_tool_runs::completed_at.desc(),
                light_tool_runs::id.desc(),
            ))
            .limit(limit)
            .select(LightToolRun::as_select())
            .load::<LightToolRun>(&mut conn)?;
        runs.reverse();
        runs.into_iter()
            .map(decrypt_run)
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn find_recent_for_principal(
        &self,
        device_id: i32,
        account_user_id: Option<i32>,
        limit: i64,
    ) -> Result<Vec<LightToolRunRecord>, LightToolRunsRepositoryError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let query = light_tool_runs::table
            .filter(light_tool_runs::device_id.eq(device_id))
            .into_boxed();
        let query = match account_user_id {
            Some(user_id) => query.filter(light_tool_runs::account_user_id.eq(user_id)),
            None => query.filter(light_tool_runs::account_user_id.is_null()),
        };
        let mut runs = query
            .order((
                light_tool_runs::created_at.desc(),
                light_tool_runs::updated_at.desc(),
                light_tool_runs::id.desc(),
            ))
            .limit(limit)
            .select(LightToolRun::as_select())
            .load::<LightToolRun>(&mut conn)?;
        runs.reverse();
        runs.into_iter()
            .map(decrypt_run)
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn claim_queued_run(
        &self,
        run_id: &str,
        activity_text: &str,
        now: i32,
    ) -> Result<Option<LightToolRunRecord>, LightToolRunsRepositoryError> {
        let encrypted_activity_text = encrypt(activity_text)?;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let run = diesel::update(
            light_tool_runs::table
                .filter(light_tool_runs::id.eq(run_id))
                .filter(light_tool_runs::status.eq("queued")),
        )
        .set((
            light_tool_runs::status.eq("running"),
            light_tool_runs::encrypted_activity_text.eq(Some(encrypted_activity_text)),
            light_tool_runs::updated_at.eq(now),
        ))
        .get_result::<LightToolRun>(&mut conn)
        .optional()?;
        Ok(run.map(decrypt_run).transpose()?)
    }

    pub fn update_running_activity(
        &self,
        run_id: &str,
        activity_text: &str,
        now: i32,
    ) -> Result<Option<LightToolRunRecord>, LightToolRunsRepositoryError> {
        let encrypted_activity_text = encrypt(activity_text)?;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let run = diesel::update(
            light_tool_runs::table
                .filter(light_tool_runs::id.eq(run_id))
                .filter(light_tool_runs::status.eq("running")),
        )
        .set((
            light_tool_runs::encrypted_activity_text.eq(Some(encrypted_activity_text)),
            light_tool_runs::updated_at.eq(now),
        ))
        .get_result::<LightToolRun>(&mut conn)
        .optional()?;
        Ok(run.map(decrypt_run).transpose()?)
    }

    pub fn complete_running_run(
        &self,
        run_id: &str,
        assistant_message: &str,
        now: i32,
    ) -> Result<Option<LightToolRunRecord>, LightToolRunsRepositoryError> {
        let encrypted_assistant_message = encrypt(assistant_message)?;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let run = diesel::update(
            light_tool_runs::table
                .filter(light_tool_runs::id.eq(run_id))
                .filter(light_tool_runs::status.eq("running")),
        )
        .set((
            light_tool_runs::status.eq("completed"),
            light_tool_runs::encrypted_activity_text.eq::<Option<String>>(None),
            light_tool_runs::encrypted_assistant_message.eq(Some(encrypted_assistant_message)),
            light_tool_runs::encrypted_error_message.eq::<Option<String>>(None),
            light_tool_runs::updated_at.eq(now),
            light_tool_runs::completed_at.eq(Some(now)),
        ))
        .get_result::<LightToolRun>(&mut conn)
        .optional()?;
        Ok(run.map(decrypt_run).transpose()?)
    }

    pub fn fail_running_run(
        &self,
        run_id: &str,
        error_message: &str,
        now: i32,
    ) -> Result<Option<LightToolRunRecord>, LightToolRunsRepositoryError> {
        let encrypted_error_message = encrypt(error_message)?;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let run = diesel::update(
            light_tool_runs::table
                .filter(light_tool_runs::id.eq(run_id))
                .filter(light_tool_runs::status.eq("running")),
        )
        .set((
            light_tool_runs::status.eq("failed"),
            light_tool_runs::encrypted_activity_text.eq::<Option<String>>(None),
            light_tool_runs::encrypted_assistant_message.eq::<Option<String>>(None),
            light_tool_runs::encrypted_error_message.eq(Some(encrypted_error_message)),
            light_tool_runs::updated_at.eq(now),
            light_tool_runs::completed_at.eq(Some(now)),
        ))
        .get_result::<LightToolRun>(&mut conn)
        .optional()?;
        Ok(run.map(decrypt_run).transpose()?)
    }

    /// Stops runs whose worker disappeared without replaying potentially
    /// side-effecting account actions such as sending a message or email.
    pub fn fail_stale_running_runs(
        &self,
        stale_before: i32,
        error_message: &str,
        now: i32,
    ) -> Result<Vec<(String, i32)>, LightToolRunsRepositoryError> {
        let encrypted_error_message = encrypt(error_message)?;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        Ok(diesel::update(
            light_tool_runs::table
                .filter(light_tool_runs::status.eq("running"))
                .filter(light_tool_runs::updated_at.le(stale_before)),
        )
        .set((
            light_tool_runs::status.eq("failed"),
            light_tool_runs::encrypted_activity_text.eq::<Option<String>>(None),
            light_tool_runs::encrypted_assistant_message.eq::<Option<String>>(None),
            light_tool_runs::encrypted_error_message.eq(Some(encrypted_error_message)),
            light_tool_runs::updated_at.eq(now),
            light_tool_runs::completed_at.eq(Some(now)),
        ))
        .returning((light_tool_runs::id, light_tool_runs::device_id))
        .get_results::<(String, i32)>(&mut conn)?)
    }

    pub fn find_queued_run_ids(
        &self,
        limit: i64,
    ) -> Result<Vec<(String, i32)>, LightToolRunsRepositoryError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        Ok(light_tool_runs::table
            .filter(light_tool_runs::status.eq("queued"))
            .order((light_tool_runs::created_at.asc(), light_tool_runs::id.asc()))
            .limit(limit)
            .select((light_tool_runs::id, light_tool_runs::device_id))
            .load::<(String, i32)>(&mut conn)?)
    }
}

fn remaining_messages(device: &LightToolDevice, message_limit: i32) -> i32 {
    (message_limit - device.trial_messages_used).max(0)
}

fn decrypt_run(run: LightToolRun) -> Result<LightToolRunRecord, EncryptionError> {
    Ok(LightToolRunRecord {
        id: run.id,
        device_id: run.device_id,
        account_user_id: run.account_user_id,
        client_message_id: run.client_message_id,
        user_message: decrypt(&run.encrypted_user_message)?,
        activity_text: decrypt_optional(run.encrypted_activity_text)?,
        assistant_message: decrypt_optional(run.encrypted_assistant_message)?,
        error_message: decrypt_optional(run.encrypted_error_message)?,
        status: run.status,
        created_at: run.created_at,
        updated_at: run.updated_at,
        completed_at: run.completed_at,
    })
}

fn decrypt_optional(value: Option<String>) -> Result<Option<String>, EncryptionError> {
    value.map(|value| decrypt(&value)).transpose()
}
