use crate::{
    repositories::{
        light_tool_push_outbox_repository::LightToolPushOutboxRepository,
        light_tool_runs_repository::{LightToolRunsRepository, LightToolRunsRepositoryError},
    },
    services::light_tool_run_dispatcher::{
        dispatch_light_tool_run, LightToolDispatchOutcome, LightToolResponder,
    },
    PgDbPool,
};
use std::{collections::HashSet, sync::Arc, time::Duration};

const POLL_INTERVAL: Duration = Duration::from_secs(15);
const STALE_RUNNING_AFTER_SECONDS: i32 = 15 * 60;
const QUEUED_BATCH_SIZE: i64 = 100;
const INTERRUPTED_ERROR: &str = "REQUEST INTERRUPTED";

pub async fn start_light_tool_run_supervisor(
    pool: PgDbPool,
    responder: Arc<dyn LightToolResponder>,
) {
    loop {
        if let Err(error) = supervise_light_tool_runs_once(pool.clone(), responder.clone()).await {
            tracing::error!("Light Tool run supervision failed: {error}");
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

pub async fn supervise_light_tool_runs_once(
    pool: PgDbPool,
    responder: Arc<dyn LightToolResponder>,
) -> Result<(), LightToolRunsRepositoryError> {
    let repository = LightToolRunsRepository::new(pool.clone());
    let now = chrono::Utc::now().timestamp() as i32;
    let interrupted = repository.fail_stale_running_runs(
        now - STALE_RUNNING_AFTER_SECONDS,
        INTERRUPTED_ERROR,
        now,
    )?;
    let queued = repository.find_queued_run_ids(QUEUED_BATCH_SIZE)?;

    for device_id in interrupted
        .into_iter()
        .map(|(_, device_id)| device_id)
        .collect::<HashSet<_>>()
    {
        enqueue_push(&pool, device_id);
    }

    for (run_id, device_id) in queued {
        let pool = pool.clone();
        let responder = responder.clone();
        tokio::spawn(async move {
            let outcome = match dispatch_light_tool_run(pool.clone(), run_id.clone(), responder)
                .await
            {
                Ok(outcome) => outcome,
                Err(error) => {
                    tracing::error!(run_id = %run_id, "Recovered Light Tool dispatch failed: {error}");
                    return;
                }
            };
            if matches!(
                outcome,
                LightToolDispatchOutcome::Completed | LightToolDispatchOutcome::Failed
            ) {
                enqueue_push(&pool, device_id);
            }
        });
    }

    Ok(())
}

fn enqueue_push(pool: &PgDbPool, device_id: i32) {
    let now = chrono::Utc::now().timestamp() as i32;
    if let Err(error) = LightToolPushOutboxRepository::new(pool.clone())
        .enqueue_conversation_changed(device_id, now)
    {
        tracing::warn!(
            device_id,
            "Could not enqueue Light Tool recovery push: {error}"
        );
    }
}
