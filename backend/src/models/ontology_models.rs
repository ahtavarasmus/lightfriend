use crate::pg_schema::{
    ont_changelog, ont_channels, ont_links, ont_messages, ont_person_edits, ont_persons, ont_rules,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

// -- ont_persons --

#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = ont_persons)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct OntPerson {
    pub id: i32,
    pub user_id: i32,
    pub name: String,
    pub created_at: i32,
    pub updated_at: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = ont_persons)]
pub struct NewOntPerson {
    pub user_id: i32,
    pub name: String,
    pub created_at: i32,
    pub updated_at: i32,
}

// -- ont_person_edits --

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = ont_person_edits)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct OntPersonEdit {
    pub id: i32,
    pub user_id: i32,
    pub person_id: i32,
    pub property_name: String,
    pub value: String,
    pub edited_at: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = ont_person_edits)]
pub struct NewOntPersonEdit {
    pub user_id: i32,
    pub person_id: i32,
    pub property_name: String,
    pub value: String,
    pub edited_at: i32,
}

// -- ont_channels --

#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = ont_channels)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct OntChannel {
    pub id: i32,
    pub user_id: i32,
    pub person_id: i32,
    pub platform: String,
    pub handle: Option<String>,
    pub room_id: Option<String>,
    pub notification_mode: String,
    pub notification_type: String,
    pub notify_on_call: i32,
    pub created_at: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = ont_channels)]
pub struct NewOntChannel {
    pub user_id: i32,
    pub person_id: i32,
    pub platform: String,
    pub handle: Option<String>,
    pub room_id: Option<String>,
    pub notification_mode: String,
    pub notification_type: String,
    pub notify_on_call: i32,
    pub created_at: i32,
}

// -- ont_changelog --

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = ont_changelog)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct OntChangelog {
    pub id: i64,
    pub user_id: i32,
    pub entity_type: String,
    pub entity_id: i32,
    pub change_type: String,
    pub changed_fields: Option<String>,
    pub source: String,
    pub created_at: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = ont_changelog)]
pub struct NewOntChangelog {
    pub user_id: i32,
    pub entity_type: String,
    pub entity_id: i32,
    pub change_type: String,
    pub changed_fields: Option<String>,
    pub source: String,
    pub created_at: i32,
}

// -- ont_links --

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = ont_links)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct OntLink {
    pub id: i32,
    pub user_id: i32,
    pub source_type: String,
    pub source_id: i32,
    pub target_type: String,
    pub target_id: i32,
    pub link_type: String,
    pub metadata: Option<String>,
    pub created_at: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = ont_links)]
pub struct NewOntLink {
    pub user_id: i32,
    pub source_type: String,
    pub source_id: i32,
    pub target_type: String,
    pub target_id: i32,
    pub link_type: String,
    pub metadata: Option<String>,
    pub created_at: i32,
}

// -- ont_messages --

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = ont_messages)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct OntMessage {
    pub id: i64,
    pub user_id: i32,
    pub room_id: String,
    pub platform: String,
    pub sender_name: String,
    pub content: String,
    pub person_id: Option<i32>,
    pub created_at: i32,
    pub pinned: bool,
    pub status: Option<String>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = ont_messages)]
pub struct NewOntMessage {
    pub user_id: i32,
    pub room_id: String,
    pub platform: String,
    pub sender_name: String,
    pub content: String,
    pub person_id: Option<i32>,
    pub created_at: i32,
    pub pinned: bool,
    pub status: Option<String>,
}

// -- ont_rules --

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = ont_rules)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct OntRule {
    pub id: i32,
    pub user_id: i32,
    pub name: String,
    pub trigger_type: String,
    pub trigger_config: String,
    pub logic_type: String,
    pub logic_prompt: Option<String>,
    pub logic_fetch: Option<String>,
    pub action_type: String,
    pub action_config: String,
    pub status: String,
    pub next_fire_at: Option<i32>,
    pub expires_at: Option<i32>,
    pub last_triggered_at: Option<i32>,
    pub created_at: i32,
    pub updated_at: i32,
    pub flow_config: Option<String>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = ont_rules)]
pub struct NewOntRule {
    pub user_id: i32,
    pub name: String,
    pub trigger_type: String,
    pub trigger_config: String,
    pub logic_type: String,
    pub logic_prompt: Option<String>,
    pub logic_fetch: Option<String>,
    pub action_type: String,
    pub action_config: String,
    pub status: String,
    pub next_fire_at: Option<i32>,
    pub expires_at: Option<i32>,
    pub created_at: i32,
    pub updated_at: i32,
    pub flow_config: Option<String>,
}

// -- Composite view types for API responses --

#[derive(Debug, Clone, Serialize)]
pub struct PersonWithChannels {
    pub person: OntPerson,
    pub channels: Vec<OntChannel>,
    pub edits: Vec<OntPersonEdit>,
}

impl PersonWithChannels {
    /// Get the effective nickname: user edit override, or base name
    pub fn display_name(&self) -> &str {
        self.edits
            .iter()
            .find(|e| e.property_name == "nickname")
            .map(|e| e.value.as_str())
            .unwrap_or(&self.person.name)
    }

    /// Get the effective notification mode for a specific channel.
    /// Channel 'default' -> Person edit -> user_settings default (caller provides fallback).
    pub fn effective_notification_mode(&self, channel: &OntChannel, user_default: &str) -> String {
        if channel.notification_mode != "default" {
            return channel.notification_mode.clone();
        }
        // Fall back to person-level edit
        self.edits
            .iter()
            .find(|e| e.property_name == "notification_mode")
            .map(|e| e.value.clone())
            .unwrap_or_else(|| user_default.to_string())
    }

    /// Get the effective notification type for a specific channel.
    pub fn effective_notification_type(&self, channel: &OntChannel, user_default: &str) -> String {
        if channel.notification_type != "sms" || channel.notification_mode != "default" {
            // If channel has explicit settings (not using defaults)
            if channel.notification_mode != "default" {
                return channel.notification_type.clone();
            }
        }
        self.edits
            .iter()
            .find(|e| e.property_name == "notification_type")
            .map(|e| e.value.clone())
            .unwrap_or_else(|| user_default.to_string())
    }

    /// Get effective notify_on_call for a channel.
    pub fn effective_notify_on_call(&self, channel: &OntChannel, user_default: bool) -> bool {
        if channel.notification_mode != "default" {
            return channel.notify_on_call != 0;
        }
        self.edits
            .iter()
            .find(|e| e.property_name == "notify_on_call")
            .map(|e| e.value == "1" || e.value == "true")
            .unwrap_or(user_default)
    }
}
