use crate::{
    repositories::light_tool_runs_repository::LightToolRunsRepository,
    services::light_tool_run_execution::{
        LightToolRunExecutionError, LightToolRunExecutionService,
    },
    PgDbPool,
};
use async_trait::async_trait;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;

const INITIAL_ACTIVITY: &str = "THINKING...";
const GENERIC_FAILURE: &str = "REQUEST FAILED";
const ACTIVITY_BUFFER: usize = 8;
const HISTORY_TURN_LIMIT: i64 = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LightToolConversationTurn {
    pub user_message: String,
    pub assistant_message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightToolRunPrincipal {
    Anonymous { device_id: i32 },
    Account { device_id: i32, user_id: i32 },
}

/// Produces one assistant reply. Activity messages must be short, safe labels
/// for the user, never private chain-of-thought or raw provider diagnostics.
#[async_trait]
pub trait LightToolResponder: Send + Sync {
    async fn respond(
        &self,
        principal: LightToolRunPrincipal,
        history: &[LightToolConversationTurn],
        user_message: &str,
        image_data_url: Option<&str>,
        activity_tx: mpsc::Sender<String>,
    ) -> Result<String, String>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightToolDispatchOutcome {
    Completed,
    Failed,
    NotClaimed,
    Superseded,
}

#[derive(Debug, Error)]
pub enum LightToolDispatchError {
    #[error(transparent)]
    Execution(#[from] LightToolRunExecutionError),
}

pub async fn dispatch_light_tool_run(
    pool: PgDbPool,
    run_id: String,
    responder: Arc<dyn LightToolResponder>,
) -> Result<LightToolDispatchOutcome, LightToolDispatchError> {
    let execution = LightToolRunExecutionService::new(LightToolRunsRepository::new(pool.clone()));
    let now = chrono::Utc::now().timestamp() as i32;
    let Some(run) = execution.claim(&run_id, INITIAL_ACTIVITY, now)? else {
        return Ok(LightToolDispatchOutcome::NotClaimed);
    };

    let prior_runs = match LightToolRunsRepository::new(pool).find_recent_completed_for_principal(
        run.device_id,
        run.account_user_id,
        run.created_at,
        HISTORY_TURN_LIMIT,
    ) {
        Ok(runs) => runs,
        Err(_) => {
            tracing::error!(run_id = %run_id, "Light Tool conversation history load failed");
            return fail_run(&execution, &run_id);
        }
    };
    let history = prior_runs
        .into_iter()
        .filter_map(|run| {
            run.assistant_message
                .map(|assistant_message| LightToolConversationTurn {
                    user_message: run.user_message,
                    assistant_message,
                })
        })
        .collect::<Vec<_>>();

    let (activity_tx, mut activity_rx) = mpsc::channel(ACTIVITY_BUFFER);
    let principal = match run.account_user_id {
        Some(user_id) => LightToolRunPrincipal::Account {
            device_id: run.device_id,
            user_id,
        },
        None => LightToolRunPrincipal::Anonymous {
            device_id: run.device_id,
        },
    };
    let response = responder.respond(
        principal,
        &history,
        &run.user_message,
        run.image_data_url.as_deref(),
        activity_tx,
    );
    tokio::pin!(response);
    let mut activity_closed = false;

    let response = loop {
        tokio::select! {
            response = &mut response => break response,
            activity = activity_rx.recv(), if !activity_closed => {
                let Some(activity) = activity else {
                    activity_closed = true;
                    continue;
                };
                let now = chrono::Utc::now().timestamp() as i32;
                match execution.update_activity(&run_id, &activity, now) {
                    Ok(Some(_)) => {}
                    Ok(None) => return Ok(LightToolDispatchOutcome::Superseded),
                    Err(LightToolRunExecutionError::BlankText { .. }
                        | LightToolRunExecutionError::TextTooLong { .. }) => {
                        tracing::warn!(run_id = %run_id, "Ignoring invalid Light Tool activity label");
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        }
    };

    match response {
        Ok(assistant_message) => {
            let now = chrono::Utc::now().timestamp() as i32;
            match execution.complete(&run_id, &assistant_message, now) {
                Ok(Some(_)) => Ok(LightToolDispatchOutcome::Completed),
                Ok(None) => Ok(LightToolDispatchOutcome::Superseded),
                Err(
                    LightToolRunExecutionError::BlankText { .. }
                    | LightToolRunExecutionError::TextTooLong { .. },
                ) => {
                    tracing::error!(run_id = %run_id, "Light Tool responder returned an invalid reply");
                    fail_run(&execution, &run_id)
                }
                Err(error) => Err(error.into()),
            }
        }
        Err(_) => {
            tracing::error!(run_id = %run_id, "Light Tool responder failed");
            fail_run(&execution, &run_id)
        }
    }
}

fn fail_run(
    execution: &LightToolRunExecutionService,
    run_id: &str,
) -> Result<LightToolDispatchOutcome, LightToolDispatchError> {
    let now = chrono::Utc::now().timestamp() as i32;
    match execution.fail(run_id, GENERIC_FAILURE, now)? {
        Some(_) => Ok(LightToolDispatchOutcome::Failed),
        None => Ok(LightToolDispatchOutcome::Superseded),
    }
}
