use crate::repositories::light_tool_runs_repository::{
    LightToolRunRecord, LightToolRunsRepository, LightToolRunsRepositoryError,
};
use thiserror::Error;

pub const MAX_RUN_ACTIVITY_CHARACTERS: usize = 80;
pub const MAX_RUN_ERROR_CHARACTERS: usize = 120;
pub const MAX_RUN_ASSISTANT_CHARACTERS: usize = 32_000;

#[derive(Debug, Error)]
pub enum LightToolRunExecutionError {
    #[error("{field} cannot be blank")]
    BlankText { field: &'static str },
    #[error("{field} must be at most {max_characters} characters")]
    TextTooLong {
        field: &'static str,
        max_characters: usize,
    },
    #[error(transparent)]
    Repository(#[from] LightToolRunsRepositoryError),
}

pub struct LightToolRunExecutionService {
    repository: LightToolRunsRepository,
}

impl LightToolRunExecutionService {
    pub fn new(repository: LightToolRunsRepository) -> Self {
        Self { repository }
    }

    pub fn claim(
        &self,
        run_id: &str,
        activity_text: &str,
        now: i32,
    ) -> Result<Option<LightToolRunRecord>, LightToolRunExecutionError> {
        let activity_text =
            normalize_text(activity_text, "activity_text", MAX_RUN_ACTIVITY_CHARACTERS)?;
        Ok(self
            .repository
            .claim_queued_run(run_id, &activity_text, now)?)
    }

    pub fn update_activity(
        &self,
        run_id: &str,
        activity_text: &str,
        now: i32,
    ) -> Result<Option<LightToolRunRecord>, LightToolRunExecutionError> {
        let activity_text =
            normalize_text(activity_text, "activity_text", MAX_RUN_ACTIVITY_CHARACTERS)?;
        Ok(self
            .repository
            .update_running_activity(run_id, &activity_text, now)?)
    }

    pub fn complete(
        &self,
        run_id: &str,
        assistant_message: &str,
        now: i32,
    ) -> Result<Option<LightToolRunRecord>, LightToolRunExecutionError> {
        let assistant_message = normalize_text(
            assistant_message,
            "assistant_message",
            MAX_RUN_ASSISTANT_CHARACTERS,
        )?;
        Ok(self
            .repository
            .complete_running_run(run_id, &assistant_message, now)?)
    }

    pub fn fail(
        &self,
        run_id: &str,
        error_message: &str,
        now: i32,
    ) -> Result<Option<LightToolRunRecord>, LightToolRunExecutionError> {
        let error_message =
            normalize_text(error_message, "error_message", MAX_RUN_ERROR_CHARACTERS)?;
        Ok(self
            .repository
            .fail_running_run(run_id, &error_message, now)?)
    }
}

fn normalize_text(
    value: &str,
    field: &'static str,
    max_characters: usize,
) -> Result<String, LightToolRunExecutionError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(LightToolRunExecutionError::BlankText { field });
    }
    if value.chars().count() > max_characters {
        return Err(LightToolRunExecutionError::TextTooLong {
            field,
            max_characters,
        });
    }
    Ok(value.to_string())
}
