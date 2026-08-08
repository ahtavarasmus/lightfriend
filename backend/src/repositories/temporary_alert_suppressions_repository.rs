use diesel::prelude::*;
use diesel::result::Error as DieselError;

use crate::models::user_models::{NewTemporaryAlertSuppression, TemporaryAlertSuppression};
use crate::pg_schema::temporary_alert_suppressions;
use crate::PgDbPool;

pub const KIND_TOPIC: &str = "topic";
pub const KIND_QUIET: &str = "quiet";
pub const SCOPE_ALL: &str = "all";
pub const SCOPE_CRITICAL: &str = "critical";
pub const SCOPE_DIGEST: &str = "digest";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuppressionDecision {
    Allow,
    SuppressQuiet { scope: String },
    SuppressTopic { match_text: String },
}

#[derive(Clone)]
pub struct TemporaryAlertSuppressionsRepository {
    pool: PgDbPool,
}

impl TemporaryAlertSuppressionsRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        user_id: i32,
        kind: &str,
        scope: &str,
        match_text: Option<&str>,
        timezone: &str,
        expires_at: i32,
    ) -> Result<TemporaryAlertSuppression, DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        conn.transaction(|conn| {
            if kind == KIND_QUIET {
                diesel::update(
                    temporary_alert_suppressions::table
                        .filter(temporary_alert_suppressions::user_id.eq(user_id))
                        .filter(temporary_alert_suppressions::kind.eq(KIND_QUIET))
                        .filter(temporary_alert_suppressions::ended_at.is_null())
                        .filter(temporary_alert_suppressions::expires_at.gt(now)),
                )
                .set(temporary_alert_suppressions::ended_at.eq(Some(now)))
                .execute(conn)?;
            }
            diesel::insert_into(temporary_alert_suppressions::table)
                .values(NewTemporaryAlertSuppression {
                    user_id,
                    kind,
                    scope,
                    match_text,
                    timezone,
                    created_at: now,
                    expires_at,
                })
                .get_result(conn)
        })
    }

    pub fn end_quiet(
        &self,
        user_id: i32,
        scope: Option<&str>,
    ) -> Result<Vec<TemporaryAlertSuppression>, DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        let base = temporary_alert_suppressions::table
            .filter(temporary_alert_suppressions::user_id.eq(user_id))
            .filter(temporary_alert_suppressions::kind.eq(KIND_QUIET))
            .filter(temporary_alert_suppressions::ended_at.is_null())
            .filter(temporary_alert_suppressions::expires_at.gt(now));
        match scope {
            Some(scope) => {
                diesel::update(base.filter(temporary_alert_suppressions::scope.eq(scope)))
                    .set(temporary_alert_suppressions::ended_at.eq(Some(now)))
                    .get_results(&mut conn)
            }
            None => diesel::update(base)
                .set(temporary_alert_suppressions::ended_at.eq(Some(now)))
                .get_results(&mut conn),
        }
    }

    pub fn active_for_user(
        &self,
        user_id: i32,
        now: i32,
    ) -> Result<Vec<TemporaryAlertSuppression>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        temporary_alert_suppressions::table
            .filter(temporary_alert_suppressions::user_id.eq(user_id))
            .filter(temporary_alert_suppressions::ended_at.is_null())
            .filter(temporary_alert_suppressions::expires_at.gt(now))
            .order(temporary_alert_suppressions::created_at.desc())
            .load(&mut conn)
    }

    pub fn has_quiet_scope(&self, user_id: i32, category: &str) -> Result<bool, DieselError> {
        let active = self.active_for_user(user_id, chrono::Utc::now().timestamp() as i32)?;
        Ok(active
            .iter()
            .any(|row| row.kind == KIND_QUIET && quiet_scope_matches(&row.scope, category)))
    }

    pub fn decision(
        &self,
        user_id: i32,
        category: &str,
        searchable_text: &str,
        always_show: bool,
    ) -> Result<SuppressionDecision, DieselError> {
        Ok(decide_suppression(
            &self.active_for_user(user_id, chrono::Utc::now().timestamp() as i32)?,
            category,
            searchable_text,
            always_show,
        ))
    }
}

pub fn decide_suppression(
    active: &[TemporaryAlertSuppression],
    category: &str,
    searchable_text: &str,
    always_show: bool,
) -> SuppressionDecision {
    // Quiet Mode is explicit and has highest precedence. Always-show only
    // overrides topic windows; it does not punch through a quiet scope.
    if let Some(row) = active
        .iter()
        .find(|row| row.kind == KIND_QUIET && quiet_scope_matches(&row.scope, category))
    {
        return SuppressionDecision::SuppressQuiet {
            scope: row.scope.clone(),
        };
    }
    if always_show || category == SCOPE_DIGEST {
        return SuppressionDecision::Allow;
    }
    active
        .iter()
        .filter(|row| row.kind == KIND_TOPIC)
        .find_map(|row| {
            let scope = row.match_text.as_deref()?;
            topic_matches(scope, searchable_text).then(|| SuppressionDecision::SuppressTopic {
                match_text: scope.to_string(),
            })
        })
        .unwrap_or(SuppressionDecision::Allow)
}

fn quiet_scope_matches(scope: &str, category: &str) -> bool {
    scope == SCOPE_ALL || scope == category
}

pub fn topic_matches(scope: &str, searchable_text: &str) -> bool {
    let haystack = searchable_text.to_lowercase();
    let significant: Vec<String> = scope
        .split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|token| token.len() >= 3 && !is_generic_scope_word(token))
        .collect();
    !significant.is_empty() && significant.iter().all(|token| haystack.contains(token))
}

fn is_generic_scope_word(token: &str) -> bool {
    matches!(
        token,
        "alert"
            | "alerts"
            | "expected"
            | "message"
            | "messages"
            | "notification"
            | "notifications"
            | "transaction"
            | "transactions"
            | "activity"
            | "about"
            | "from"
    )
}
