use crate::models::agent_integration_models::{
    AgentCredential, AgentPairingSession, NewAgentActionAudit, NewAgentActionIdempotency,
    NewAgentCredential, NewAgentPairingSession,
};
use crate::models::user_models::NewPendingReplyWatch;
use crate::pg_schema::{
    agent_action_audit, agent_action_idempotency, agent_credentials, agent_pairing_sessions,
    imap_connection, pending_reply_watches, users,
};
use crate::PgDbPool;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use std::collections::HashSet;

const IDEMPOTENCY_TTL_SECONDS: i32 = 86_400;

pub struct AgentIntegrationRepository {
    pool: PgDbPool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PairingPoll {
    Pending,
    Invalid,
    Approved { user_id: i32, label: String },
}

#[derive(Debug)]
pub enum CredentialClaim {
    Accepted(AgentCredential),
    Invalid,
    OverCap,
}

#[derive(Debug, PartialEq, Eq)]
pub enum IdempotencyClaim {
    Fresh(i32),
    Replayed(String),
    InFlight,
}

pub struct CredentialIssue<'a> {
    pub user_id: i32,
    pub token_hash: &'a str,
    pub token_prefix: &'a str,
    pub label: &'a str,
    pub issued_at: i32,
    pub expires_at: i32,
}

impl AgentIntegrationRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    pub fn create_pairing(
        &self,
        device_code_hash: &str,
        user_code_hash: &str,
        client_name: &str,
        now: i32,
        expires_at: i32,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().map_err(|_| DieselError::NotFound)?;
        conn.transaction(|conn| {
            diesel::delete(
                agent_pairing_sessions::table.filter(
                    agent_pairing_sessions::expires_at
                        .le(now)
                        .or(agent_pairing_sessions::consumed_at.is_not_null()),
                ),
            )
            .execute(conn)?;
            diesel::insert_into(agent_pairing_sessions::table)
                .values(NewAgentPairingSession {
                    device_code_hash,
                    user_code_hash,
                    client_name,
                    created_at: now,
                    expires_at,
                })
                .execute(conn)?;
            Ok(())
        })
    }

    pub fn approve_pairing(
        &self,
        user_id: i32,
        user_code_hash: &str,
        now: i32,
    ) -> Result<Option<String>, DieselError> {
        let mut conn = self.pool.get().map_err(|_| DieselError::NotFound)?;
        conn.transaction(|conn| {
            let session = agent_pairing_sessions::table
                .filter(agent_pairing_sessions::user_code_hash.eq(user_code_hash))
                .filter(agent_pairing_sessions::expires_at.gt(now))
                .filter(agent_pairing_sessions::approved_at.is_null())
                .filter(agent_pairing_sessions::consumed_at.is_null())
                .for_update()
                .select(AgentPairingSession::as_select())
                .first::<AgentPairingSession>(conn)
                .optional()?;
            let Some(session) = session else {
                return Ok(None);
            };
            diesel::update(agent_pairing_sessions::table.find(session.id))
                .set((
                    agent_pairing_sessions::approved_by_user_id.eq(Some(user_id)),
                    agent_pairing_sessions::approved_at.eq(Some(now)),
                ))
                .execute(conn)?;
            Ok(Some(session.client_name))
        })
    }

    pub fn poll_pairing(
        &self,
        device_code_hash: &str,
        now: i32,
    ) -> Result<PairingPoll, DieselError> {
        let mut conn = self.pool.get().map_err(|_| DieselError::NotFound)?;
        let session = agent_pairing_sessions::table
            .filter(agent_pairing_sessions::device_code_hash.eq(device_code_hash))
            .select(AgentPairingSession::as_select())
            .first::<AgentPairingSession>(&mut conn)
            .optional()?;
        let Some(session) = session else {
            return Ok(PairingPoll::Invalid);
        };
        if session.expires_at <= now || session.consumed_at.is_some() {
            return Ok(PairingPoll::Invalid);
        }
        match session.approved_by_user_id {
            Some(user_id) => Ok(PairingPoll::Approved {
                user_id,
                label: session.client_name,
            }),
            None => Ok(PairingPoll::Pending),
        }
    }

    pub fn consume_pairing(
        &self,
        device_code_hash: &str,
        issue: CredentialIssue<'_>,
    ) -> Result<Option<AgentCredential>, DieselError> {
        let mut conn = self.pool.get().map_err(|_| DieselError::NotFound)?;
        conn.transaction(|conn| {
            let session = agent_pairing_sessions::table
                .filter(agent_pairing_sessions::device_code_hash.eq(device_code_hash))
                .filter(agent_pairing_sessions::approved_by_user_id.eq(Some(issue.user_id)))
                .filter(agent_pairing_sessions::expires_at.gt(issue.issued_at))
                .filter(agent_pairing_sessions::consumed_at.is_null())
                .for_update()
                .select(AgentPairingSession::as_select())
                .first::<AgentPairingSession>(conn)
                .optional()?;
            if session.is_none() {
                return Ok(None);
            }
            users::table
                .find(issue.user_id)
                .select(users::id)
                .for_update()
                .first::<i32>(conn)?;
            let active = agent_credentials::table
                .filter(agent_credentials::user_id.eq(issue.user_id))
                .filter(agent_credentials::revoked_at.is_null())
                .filter(agent_credentials::expires_at.gt(issue.issued_at))
                .count()
                .get_result::<i64>(conn)?;
            if active >= 5 {
                return Ok(None);
            }
            let credential = diesel::insert_into(agent_credentials::table)
                .values(NewAgentCredential {
                    user_id: issue.user_id,
                    token_hash: issue.token_hash,
                    token_prefix: issue.token_prefix,
                    label: issue.label,
                    scopes: "reminders,reply_watch_email",
                    daily_cap: 20,
                    daily_used: 0,
                    daily_reset_at: next_utc_midnight(issue.issued_at),
                    expires_at: issue.expires_at,
                    created_at: issue.issued_at,
                })
                .get_result::<AgentCredential>(conn)?;
            diesel::update(
                agent_pairing_sessions::table
                    .filter(agent_pairing_sessions::device_code_hash.eq(device_code_hash)),
            )
            .set(agent_pairing_sessions::consumed_at.eq(Some(issue.issued_at)))
            .execute(conn)?;
            Ok(Some(credential))
        })
    }

    pub fn list_credentials(
        &self,
        user_id: i32,
        now: i32,
    ) -> Result<Vec<AgentCredential>, DieselError> {
        let mut conn = self.pool.get().map_err(|_| DieselError::NotFound)?;
        agent_credentials::table
            .filter(agent_credentials::user_id.eq(user_id))
            .filter(agent_credentials::revoked_at.is_null())
            .filter(agent_credentials::expires_at.gt(now))
            .order(agent_credentials::created_at.desc())
            .select(AgentCredential::as_select())
            .load(&mut conn)
    }

    pub fn revoke_credential(&self, user_id: i32, id: i32, now: i32) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().map_err(|_| DieselError::NotFound)?;
        let count = diesel::update(
            agent_credentials::table
                .filter(agent_credentials::id.eq(id))
                .filter(agent_credentials::user_id.eq(user_id))
                .filter(agent_credentials::revoked_at.is_null()),
        )
        .set(agent_credentials::revoked_at.eq(Some(now)))
        .execute(&mut conn)?;
        Ok(count > 0)
    }

    pub fn revoke_by_token_hash(&self, token_hash: &str, now: i32) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().map_err(|_| DieselError::NotFound)?;
        let count = diesel::update(
            agent_credentials::table
                .filter(agent_credentials::token_hash.eq(token_hash))
                .filter(agent_credentials::revoked_at.is_null()),
        )
        .set(agent_credentials::revoked_at.eq(Some(now)))
        .execute(&mut conn)?;
        Ok(count > 0)
    }

    pub fn authenticate_credential(
        &self,
        token_hash: &str,
        required_scope: &str,
        now: i32,
    ) -> Result<Option<AgentCredential>, DieselError> {
        let mut conn = self.pool.get().map_err(|_| DieselError::NotFound)?;
        let credential = agent_credentials::table
            .filter(agent_credentials::token_hash.eq(token_hash))
            .filter(agent_credentials::revoked_at.is_null())
            .filter(agent_credentials::expires_at.gt(now))
            .select(AgentCredential::as_select())
            .first::<AgentCredential>(&mut conn)
            .optional()?;
        Ok(credential.filter(|credential| {
            credential
                .scopes
                .split(',')
                .any(|scope| scope == required_scope)
        }))
    }

    pub fn claim_credential(
        &self,
        token_hash: &str,
        required_scope: &str,
        now: i32,
    ) -> Result<CredentialClaim, DieselError> {
        let mut conn = self.pool.get().map_err(|_| DieselError::NotFound)?;
        conn.transaction(|conn| {
            diesel::update(
                agent_credentials::table
                    .filter(agent_credentials::token_hash.eq(token_hash))
                    .filter(agent_credentials::daily_reset_at.le(now)),
            )
            .set((
                agent_credentials::daily_used.eq(0),
                agent_credentials::daily_reset_at.eq(next_utc_midnight(now)),
            ))
            .execute(conn)?;
            let Some(current) = agent_credentials::table
                .filter(agent_credentials::token_hash.eq(token_hash))
                .select(AgentCredential::as_select())
                .first::<AgentCredential>(conn)
                .optional()?
            else {
                return Ok(CredentialClaim::Invalid);
            };
            if current.revoked_at.is_some()
                || current.expires_at <= now
                || !current
                    .scopes
                    .split(',')
                    .any(|scope| scope == required_scope)
            {
                return Ok(CredentialClaim::Invalid);
            }
            let updated = diesel::update(
                agent_credentials::table
                    .find(current.id)
                    .filter(agent_credentials::revoked_at.is_null())
                    .filter(agent_credentials::expires_at.gt(now))
                    .filter(agent_credentials::daily_used.lt(agent_credentials::daily_cap)),
            )
            .set((
                agent_credentials::daily_used.eq(agent_credentials::daily_used + 1),
                agent_credentials::last_used_at.eq(Some(now)),
            ))
            .get_result::<AgentCredential>(conn)
            .optional()?;
            Ok(match updated {
                Some(credential) => CredentialClaim::Accepted(credential),
                None => CredentialClaim::OverCap,
            })
        })
    }

    pub fn reserve_idempotency(
        &self,
        credential_id: i32,
        action_kind: &str,
        key_hash: &str,
        now: i32,
    ) -> Result<IdempotencyClaim, DieselError> {
        let mut conn = self.pool.get().map_err(|_| DieselError::NotFound)?;
        conn.transaction(|conn| {
            let existing = agent_action_idempotency::table
                .filter(agent_action_idempotency::credential_id.eq(credential_id))
                .filter(agent_action_idempotency::action_kind.eq(action_kind))
                .filter(agent_action_idempotency::key_hash.eq(key_hash))
                .select((
                    agent_action_idempotency::id,
                    agent_action_idempotency::outcome,
                    agent_action_idempotency::created_at,
                ))
                .first::<(i32, Option<String>, i32)>(conn)
                .optional()?;
            if let Some((id, outcome, created_at)) = existing {
                if now - created_at <= IDEMPOTENCY_TTL_SECONDS {
                    return Ok(match outcome {
                        Some(outcome) => IdempotencyClaim::Replayed(outcome),
                        None => IdempotencyClaim::InFlight,
                    });
                }
                diesel::delete(agent_action_idempotency::table.find(id)).execute(conn)?;
            }
            let insert = diesel::insert_into(agent_action_idempotency::table)
                .values(NewAgentActionIdempotency {
                    credential_id,
                    action_kind,
                    key_hash,
                    outcome: None,
                    created_at: now,
                })
                .returning(agent_action_idempotency::id)
                .get_result::<i32>(conn);
            match insert {
                Ok(id) => Ok(IdempotencyClaim::Fresh(id)),
                Err(DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => {
                    Ok(IdempotencyClaim::InFlight)
                }
                Err(error) => Err(error),
            }
        })
    }

    pub fn complete_idempotency(&self, id: i32, outcome: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().map_err(|_| DieselError::NotFound)?;
        diesel::update(agent_action_idempotency::table.find(id))
            .set(agent_action_idempotency::outcome.eq(Some(outcome)))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn clear_idempotency(&self, id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().map_err(|_| DieselError::NotFound)?;
        diesel::delete(agent_action_idempotency::table.find(id)).execute(&mut conn)?;
        Ok(())
    }

    pub fn audit(
        &self,
        credential_id: i32,
        user_id: i32,
        action_kind: &str,
        outcome: &str,
        now: i32,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().map_err(|_| DieselError::NotFound)?;
        diesel::insert_into(agent_action_audit::table)
            .values(NewAgentActionAudit {
                credential_id: Some(credential_id),
                user_id,
                action_kind,
                outcome,
                created_at: now,
            })
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn active_imap_connection_ids(&self, user_id: i32) -> Result<Vec<i32>, DieselError> {
        let mut conn = self.pool.get().map_err(|_| DieselError::NotFound)?;
        imap_connection::table
            .filter(imap_connection::user_id.eq(user_id))
            .filter(imap_connection::status.eq("active"))
            .select(imap_connection::id)
            .load(&mut conn)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_email_reply_watches(
        &self,
        user_id: i32,
        connection_ids: &[i32],
        email: &str,
        label: &str,
        now: i32,
        expires_at: i32,
        max_active: i64,
    ) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().map_err(|_| DieselError::NotFound)?;
        conn.transaction(|conn| {
            // Serialize the user-wide active-watch cap even when two of the
            // user's credentials race one another.
            users::table
                .find(user_id)
                .select(users::id)
                .for_update()
                .first::<i32>(conn)?;
            let active_rows = pending_reply_watches::table
                .filter(pending_reply_watches::user_id.eq(user_id))
                .filter(pending_reply_watches::expires_at.gt(now))
                .select((
                    pending_reply_watches::platform,
                    pending_reply_watches::contact_identifier,
                    pending_reply_watches::room_id,
                ))
                .load::<(String, String, Option<String>)>(conn)?;
            let active_groups = active_rows
                .into_iter()
                .map(|(platform, contact, room)| {
                    if platform == "email" {
                        format!("email:{contact}")
                    } else {
                        format!("{platform}:{}", room.unwrap_or(contact))
                    }
                })
                .collect::<HashSet<_>>();
            let email_group = format!("email:{email}");
            if connection_ids.is_empty()
                || active_groups.contains(&email_group)
                || active_groups.len() as i64 >= max_active
            {
                return Ok(false);
            }
            for connection_id in connection_ids {
                diesel::insert_into(pending_reply_watches::table)
                    .values(NewPendingReplyWatch {
                        user_id,
                        platform: "email".to_string(),
                        room_id: None,
                        imap_connection_id: Some(*connection_id),
                        contact_identifier: email.to_string(),
                        contact_display_name: label.to_string(),
                        created_at: now,
                        expires_at,
                    })
                    .execute(conn)?;
            }
            Ok(true)
        })
    }
}

fn next_utc_midnight(now: i32) -> i32 {
    ((now / 86_400) + 1) * 86_400
}
