use crate::{
    models::light_tool_models::NewLightToolPushOutboxEvent,
    pg_schema::{
        light_tool_devices, light_tool_push_outbox, light_tool_push_registrations, light_tool_runs,
    },
    repositories::light_tool_push_outbox_repository::CONVERSATION_CHANGED_EVENT,
    utils::encryption::{encrypt, EncryptionError},
    PgDbPool,
};
use diesel::result::Error as DieselError;
use diesel::{prelude::*, upsert::excluded};
use thiserror::Error;
use uuid::Uuid;

const NOTIFICATION_CLIENT_MESSAGE_PREFIX: &str = "notification:";

#[derive(Debug, Error)]
pub enum LightToolNotificationRepositoryError {
    #[error(transparent)]
    Database(#[from] DieselError),
    #[error(transparent)]
    Encryption(#[from] EncryptionError),
}

pub struct LightToolNotificationRepository {
    pool: PgDbPool,
}

impl LightToolNotificationRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    /// Adds the notification to every active, push-enabled Light Phone linked
    /// to the account and atomically schedules the privacy-safe refresh push.
    pub fn enqueue_for_user(
        &self,
        user_id: i32,
        notification: &str,
        now: i32,
    ) -> Result<Vec<i32>, LightToolNotificationRepositoryError> {
        let encrypted_user_message = encrypt("")?;
        let encrypted_assistant_message = encrypt(notification)?;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        conn.transaction::<Vec<i32>, LightToolNotificationRepositoryError, _>(|conn| {
            let device_ids = light_tool_devices::table
                .inner_join(light_tool_push_registrations::table)
                .filter(light_tool_devices::user_id.eq(Some(user_id)))
                .filter(light_tool_devices::revoked_at.is_null())
                .select(light_tool_devices::id)
                .load::<i32>(conn)?;

            for device_id in &device_ids {
                let run_id = Uuid::new_v4().to_string();
                diesel::insert_into(light_tool_runs::table)
                    .values((
                        light_tool_runs::id.eq(&run_id),
                        light_tool_runs::device_id.eq(device_id),
                        light_tool_runs::account_user_id.eq(Some(user_id)),
                        light_tool_runs::client_message_id
                            .eq(format!("{NOTIFICATION_CLIENT_MESSAGE_PREFIX}{run_id}")),
                        light_tool_runs::encrypted_user_message.eq(&encrypted_user_message),
                        light_tool_runs::encrypted_assistant_message
                            .eq(Some(&encrypted_assistant_message)),
                        light_tool_runs::status.eq("completed"),
                        light_tool_runs::created_at.eq(now),
                        light_tool_runs::updated_at.eq(now),
                        light_tool_runs::completed_at.eq(Some(now)),
                    ))
                    .execute(conn)?;

                let event = NewLightToolPushOutboxEvent {
                    device_id: *device_id,
                    event_type: CONVERSATION_CHANGED_EVENT.to_string(),
                    version: 1,
                    attempt_count: 0,
                    next_attempt_at: now,
                    lease_until: 0,
                    created_at: now,
                    updated_at: now,
                };
                diesel::insert_into(light_tool_push_outbox::table)
                    .values(&event)
                    .on_conflict(light_tool_push_outbox::device_id)
                    .do_update()
                    .set((
                        light_tool_push_outbox::event_type
                            .eq(excluded(light_tool_push_outbox::event_type)),
                        light_tool_push_outbox::version.eq(light_tool_push_outbox::version + 1_i64),
                        light_tool_push_outbox::attempt_count.eq(0),
                        light_tool_push_outbox::next_attempt_at
                            .eq(excluded(light_tool_push_outbox::next_attempt_at)),
                        light_tool_push_outbox::lease_until.eq(0),
                        light_tool_push_outbox::updated_at
                            .eq(excluded(light_tool_push_outbox::updated_at)),
                    ))
                    .execute(conn)?;
            }

            Ok(device_ids)
        })
    }
}

pub fn is_light_tool_notification_client_message_id(value: &str) -> bool {
    value.starts_with(NOTIFICATION_CLIENT_MESSAGE_PREFIX)
}
