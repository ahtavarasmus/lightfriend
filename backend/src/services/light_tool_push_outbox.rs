use crate::{
    repositories::light_tool_push_outbox_repository::{
        LightToolPushOutboxRepository, LightToolPushOutboxRepositoryError,
        CONVERSATION_CHANGED_EVENT,
    },
    services::light_tool_push_delivery::{
        LightToolPushDeliveryError, LightToolPushDeliveryOutcome, LightToolPushDeliveryService,
    },
    PgDbPool,
};
use futures::stream::{self, StreamExt};
use std::time::Duration;
use thiserror::Error;

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const CLAIM_BATCH_SIZE: i64 = 50;
const MAX_CONCURRENT_DELIVERIES: usize = 10;
const LEASE_SECONDS: i32 = 120;
const RETRY_DELAYS_SECONDS: [i32; 5] = [5, 15, 60, 300, 900];

#[derive(Debug, Error)]
pub enum LightToolPushOutboxError {
    #[error(transparent)]
    Repository(#[from] LightToolPushOutboxRepositoryError),
    #[error(transparent)]
    Delivery(#[from] LightToolPushDeliveryError),
}

pub struct LightToolPushOutboxWorker {
    repository: LightToolPushOutboxRepository,
    delivery: LightToolPushDeliveryService,
}

impl LightToolPushOutboxWorker {
    pub fn from_env(pool: PgDbPool) -> Result<Self, LightToolPushOutboxError> {
        Ok(Self {
            repository: LightToolPushOutboxRepository::new(pool.clone()),
            delivery: LightToolPushDeliveryService::from_env(pool)?,
        })
    }

    pub fn new<I, S>(pool: PgDbPool, allowed_hosts: I) -> Result<Self, LightToolPushOutboxError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Ok(Self {
            repository: LightToolPushOutboxRepository::new(pool.clone()),
            delivery: LightToolPushDeliveryService::new(pool, allowed_hosts)?,
        })
    }

    pub async fn process_due_once(&self, now: i32) -> Result<usize, LightToolPushOutboxError> {
        let events =
            self.repository
                .claim_due(now, now.saturating_add(LEASE_SECONDS), CLAIM_BATCH_SIZE)?;
        let event_count = events.len();

        stream::iter(events)
            .for_each_concurrent(MAX_CONCURRENT_DELIVERIES, |event| async move {
                if event.event_type != CONVERSATION_CHANGED_EVENT {
                    tracing::error!(
                        device_id = event.device_id,
                        event_version = event.version,
                        event_type = %event.event_type,
                        "Discarding unsupported Light Tool push outbox event"
                    );
                    if let Err(error) = self.repository.complete(event.device_id, event.version) {
                        tracing::error!(
                            device_id = event.device_id,
                            event_version = event.version,
                            "Could not discard unsupported Light Tool push event: {error}"
                        );
                    }
                    return;
                }

                match self
                    .delivery
                    .send_conversation_changed(event.device_id)
                    .await
                {
                    Ok(outcome) => {
                        match outcome {
                            LightToolPushDeliveryOutcome::Delivered => tracing::debug!(
                                device_id = event.device_id,
                                event_version = event.version,
                                attempt = event.attempt_count + 1,
                                "Light Tool push delivered"
                            ),
                            LightToolPushDeliveryOutcome::NoEndpoint => tracing::debug!(
                                device_id = event.device_id,
                                event_version = event.version,
                                "Light Tool push skipped because the device has no endpoint"
                            ),
                            LightToolPushDeliveryOutcome::EndpointExpired => tracing::info!(
                                device_id = event.device_id,
                                event_version = event.version,
                                "Light Tool push endpoint expired"
                            ),
                        }
                        if let Err(error) = self.repository.complete(event.device_id, event.version)
                        {
                            tracing::error!(
                                device_id = event.device_id,
                                event_version = event.version,
                                "Could not complete Light Tool push event: {error}"
                            );
                        }
                    }
                    Err(error) => {
                        let delay_seconds = retry_delay_seconds(
                            event.device_id,
                            event.version,
                            event.attempt_count,
                        );
                        let next_attempt_at = now.saturating_add(delay_seconds);
                        match self.repository.schedule_retry(
                            event.device_id,
                            event.version,
                            next_attempt_at,
                            now,
                        ) {
                            Ok(true) => tracing::warn!(
                                device_id = event.device_id,
                                event_version = event.version,
                                attempt = event.attempt_count + 1,
                                retry_in_seconds = delay_seconds,
                                "Light Tool push failed and will be retried: {error}"
                            ),
                            Ok(false) => tracing::debug!(
                                device_id = event.device_id,
                                event_version = event.version,
                                "Light Tool push event changed while delivery was in flight"
                            ),
                            Err(repository_error) => tracing::error!(
                                device_id = event.device_id,
                                event_version = event.version,
                                "Could not schedule Light Tool push retry: {repository_error}"
                            ),
                        }
                    }
                }
            })
            .await;

        Ok(event_count)
    }
}

pub async fn start_light_tool_push_outbox_worker(pool: PgDbPool) {
    let worker = match LightToolPushOutboxWorker::from_env(pool) {
        Ok(worker) => worker,
        Err(error) => {
            tracing::error!("Light Tool push outbox worker could not start: {error}");
            return;
        }
    };

    loop {
        match worker
            .process_due_once(chrono::Utc::now().timestamp() as i32)
            .await
        {
            Ok(processed) if processed == CLAIM_BATCH_SIZE as usize => continue,
            Ok(_) => {}
            Err(error) => tracing::error!("Light Tool push outbox processing failed: {error}"),
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

fn retry_delay_seconds(device_id: i32, event_version: i64, attempt_count: i32) -> i32 {
    let index = usize::try_from(attempt_count)
        .unwrap_or(usize::MAX)
        .min(RETRY_DELAYS_SECONDS.len() - 1);
    let base = RETRY_DELAYS_SECONDS[index];
    let jitter_range = (base / 5).max(1);
    let seed = i64::from(device_id)
        .wrapping_mul(31)
        .wrapping_add(event_version)
        .wrapping_add(i64::from(attempt_count));
    base.saturating_add((seed.unsigned_abs() % jitter_range as u64) as i32)
}
