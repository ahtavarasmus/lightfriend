use crate::pg_schema::{
    ont_changelog, ont_channels, ont_events, ont_links, ont_messages, ont_person_edits,
    ont_persons, ont_rules,
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
    pub urgency: Option<String>,
    pub category: Option<String>,
    pub summary: Option<String>,
    pub digest_delivered_at: Option<i32>,
    pub classification_prompt: Option<String>,
    pub classification_result: Option<String>,
    pub resolved_at: Option<i32>,
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
}

// -- ont_events --

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = ont_events)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct OntEvent {
    pub id: i32,
    pub user_id: i32,
    pub description: String,
    pub remind_at: Option<i32>,
    pub due_at: Option<i32>,
    pub status: String,
    pub created_at: i32,
    pub updated_at: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = ont_events)]
pub struct NewOntEvent {
    pub user_id: i32,
    pub description: String,
    pub remind_at: Option<i32>,
    pub due_at: Option<i32>,
    pub status: String,
    pub created_at: i32,
    pub updated_at: i32,
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

// -- Bayesian signal primitives --

pub struct BayesianEstimate {
    pub value: f64,      // posterior estimate in original units (e.g. seconds)
    pub confidence: f64, // 0.0 to 1.0
    pub n: i64,          // number of observations
}

pub struct UserBaseline {
    pub response_time_secs: f64, // geometric mean across all contacts (90-day window)
    pub total_replies: i64,      // how many reply observations the baseline is built from
}

/// Geometric mean of positive values. Returns None if empty.
pub fn geometric_mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let log_sum: f64 = values.iter().map(|v| v.ln()).sum();
    Some((log_sum / values.len() as f64).exp())
}

/// Bayesian estimate blended in log-space for log-normal data (response times).
/// Prior and observed are in original units (seconds). Blending in ln-space
/// preserves outlier resistance from the geometric mean.
pub fn bayesian_log_mean(observed_geomean: f64, n: i64, prior: f64, k: f64) -> BayesianEstimate {
    let n_f = n as f64;
    let log_est = (k * prior.ln() + n_f * observed_geomean.ln()) / (k + n_f);
    BayesianEstimate {
        value: log_est.exp(),
        confidence: n_f / (n_f + k),
        n,
    }
}

// -- Sender signals for importance evaluation --

pub struct SenderSignals {
    pub message_count_30d: i64,
    pub last_contact_ago_secs: Option<i64>,
    pub response_time: BayesianEstimate, // Bayesian, blended with user baseline in log-space
    pub baseline_response_time_secs: f64, // user's baseline for computing delta
    pub temporal_anomaly: Option<String>,
    pub platform_count: i32,
    pub is_first_contact: bool,
    pub has_custom_settings: bool,
}

impl SenderSignals {
    pub fn empty() -> Self {
        Self {
            message_count_30d: 0,
            last_contact_ago_secs: None,
            response_time: BayesianEstimate {
                value: 0.0,
                confidence: 0.0,
                n: 0,
            },
            baseline_response_time_secs: 0.0,
            temporal_anomaly: None,
            platform_count: 0,
            is_first_contact: true,
            has_custom_settings: false,
        }
    }

    pub fn format_for_prompt(&self, sender_name: &str) -> String {
        if self.message_count_30d == 0 {
            return format!(
                "No message history with {} in the last 30 days.",
                sender_name
            );
        }

        let mut parts = Vec::new();

        // Message count (raw observation, not bucketed)
        parts.push(format!(
            "{} has sent {} messages in the last 30 days.",
            sender_name, self.message_count_30d
        ));

        // Last contact
        if let Some(ago) = self.last_contact_ago_secs {
            let desc = if ago < 3600 {
                "less than an hour ago".to_string()
            } else if ago < 86400 {
                format!("{} hours ago", ago / 3600)
            } else {
                format!("{} days ago", ago / 86400)
            };
            parts.push(format!("Their previous message was {}.", desc));
        }

        // Response time relative to baseline (confidence-gated)
        if self.response_time.confidence >= 0.2 && self.baseline_response_time_secs > 0.0 {
            let ratio = self.baseline_response_time_secs / self.response_time.value;
            let qualifier = if self.response_time.confidence < 0.5 {
                "Early pattern (limited data): "
            } else {
                ""
            };

            if ratio >= 1.5 {
                parts.push(format!(
                    "{}You respond to this person ~{:.1}x faster than your average contact.",
                    qualifier, ratio
                ));
            } else if ratio <= 0.67 {
                let inverse = 1.0 / ratio;
                parts.push(format!(
                    "{}You respond to this person ~{:.1}x slower than your average contact.",
                    qualifier, inverse
                ));
            }
            // ratio between 0.67-1.5: roughly average, don't surface
        }

        // First contact
        if self.is_first_contact {
            parts.push(
                "This is a first-time or very rare sender - could be spam or genuinely urgent."
                    .to_string(),
            );
        }

        // Custom settings
        if self.has_custom_settings {
            parts.push(
                "You have configured custom notification settings for this person.".to_string(),
            );
        }

        // Multi-platform presence
        if self.platform_count >= 2 {
            parts.push(format!(
                "This person contacts you on {} platforms.",
                self.platform_count
            ));
        }

        // Temporal anomaly
        if let Some(ref anomaly) = self.temporal_anomaly {
            parts.push(anomaly.clone());
        }

        parts.join(" ")
    }
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
