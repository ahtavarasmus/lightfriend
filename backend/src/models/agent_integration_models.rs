use crate::pg_schema::{
    agent_action_audit, agent_action_idempotency, agent_credentials, agent_pairing_sessions,
};
use diesel::prelude::*;

#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = agent_credentials)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AgentCredential {
    pub id: i32,
    pub user_id: i32,
    pub token_hash: String,
    pub token_prefix: String,
    pub label: String,
    pub scopes: String,
    pub daily_cap: i32,
    pub daily_used: i32,
    pub daily_reset_at: i32,
    pub expires_at: i32,
    pub created_at: i32,
    pub last_used_at: Option<i32>,
    pub revoked_at: Option<i32>,
}

#[derive(Insertable)]
#[diesel(table_name = agent_credentials)]
pub struct NewAgentCredential<'a> {
    pub user_id: i32,
    pub token_hash: &'a str,
    pub token_prefix: &'a str,
    pub label: &'a str,
    pub scopes: &'a str,
    pub daily_cap: i32,
    pub daily_used: i32,
    pub daily_reset_at: i32,
    pub expires_at: i32,
    pub created_at: i32,
}

#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = agent_pairing_sessions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AgentPairingSession {
    pub id: i32,
    pub device_code_hash: String,
    pub user_code_hash: String,
    pub client_name: String,
    pub created_at: i32,
    pub expires_at: i32,
    pub approved_by_user_id: Option<i32>,
    pub approved_at: Option<i32>,
    pub consumed_at: Option<i32>,
}

#[derive(Insertable)]
#[diesel(table_name = agent_pairing_sessions)]
pub struct NewAgentPairingSession<'a> {
    pub device_code_hash: &'a str,
    pub user_code_hash: &'a str,
    pub client_name: &'a str,
    pub created_at: i32,
    pub expires_at: i32,
}

#[derive(Insertable)]
#[diesel(table_name = agent_action_idempotency)]
pub struct NewAgentActionIdempotency<'a> {
    pub credential_id: i32,
    pub action_kind: &'a str,
    pub key_hash: &'a str,
    pub outcome: Option<&'a str>,
    pub created_at: i32,
}

#[derive(Insertable)]
#[diesel(table_name = agent_action_audit)]
pub struct NewAgentActionAudit<'a> {
    pub credential_id: Option<i32>,
    pub user_id: i32,
    pub action_kind: &'a str,
    pub outcome: &'a str,
    pub created_at: i32,
}
