use crate::pg_schema::{commitment_label_embeddings, commitment_prompts, commitment_sender_rules};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

// -- commitment_sender_rules --

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = commitment_sender_rules)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CommitmentSenderRule {
    pub id: i32,
    pub user_id: i32,
    pub platform: String,
    pub sender_key: String,
    pub rule_type: String,
    pub source: String,
    pub active: bool,
    pub created_at: i32,
    pub deactivated_at: Option<i32>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = commitment_sender_rules)]
pub struct NewCommitmentSenderRule {
    pub user_id: i32,
    pub platform: String,
    pub sender_key: String,
    pub rule_type: String,
    pub source: String,
    pub active: bool,
    pub created_at: i32,
}

// -- commitment_label_embeddings --

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = commitment_label_embeddings)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CommitmentLabelEmbedding {
    pub id: i32,
    pub user_id: i32,
    pub label_type: String,
    pub embedding: Vec<u8>,
    pub source_message_id: Option<i64>,
    pub created_at: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = commitment_label_embeddings)]
pub struct NewCommitmentLabelEmbedding {
    pub user_id: i32,
    pub label_type: String,
    pub embedding: Vec<u8>,
    pub source_message_id: Option<i64>,
    pub created_at: i32,
}

// -- commitment_prompts --

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = commitment_prompts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CommitmentPrompt {
    pub id: i32,
    pub user_id: i32,
    pub ont_message_id: i64,
    pub platform: String,
    pub sender_key: String,
    pub sender_display_name: String,
    pub commitment_description: String,
    pub due_at: Option<i32>,
    pub remind_at: Option<i32>,
    pub sent_at: i32,
    pub sms_message_sid: Option<String>,
    pub user_label: Option<String>,
    pub labeled_at: Option<i32>,
    pub resulting_event_id: Option<i32>,
    pub resolved_at: Option<i32>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = commitment_prompts)]
pub struct NewCommitmentPrompt {
    pub user_id: i32,
    pub ont_message_id: i64,
    pub platform: String,
    pub sender_key: String,
    pub sender_display_name: String,
    pub commitment_description: String,
    pub due_at: Option<i32>,
    pub remind_at: Option<i32>,
    pub sent_at: i32,
    pub sms_message_sid: Option<String>,
}
