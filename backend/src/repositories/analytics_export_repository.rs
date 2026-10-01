//! Content-free analytics archive. Every SQL projection is an explicit allowlist;
//! never replace one with SELECT * or serialize a database/user model here.
use diesel::{connection::SimpleConnection, prelude::*, sql_types::Text};
use serde::Serialize;
use serde_json::{json, Value};
use std::io::{BufWriter, Seek, SeekFrom, Write};

use crate::services::model_pricing::{CostSnapshot, ModelRates};
use crate::PgDbPool;

struct Dataset {
    name: &'static str,
    query: &'static str,
    timestamp: Option<&'static str>,
    description: &'static str,
}

// Do not join historical rows to users: retained usage from deleted accounts
// must survive even when the email lookup no longer exists.
const DATASETS: &[Dataset] = &[
    Dataset {
        name: "users",
        query: "SELECT id AS user_id, email, sub_tier, plan_type, own_twilio_enabled,
            preferred_sms_provider, credits, credits_left, stripe_customer_id,
            next_billing_date_timestamp, included_usage_window_start_timestamp,
            included_usage_window_end_timestamp FROM users ORDER BY id",
        timestamp: None,
        description: "Current account/email lookup and balances, not subscription history. Stripe customer IDs allow joining provider payment exports. Account creation dates are not stored.",
    },
    Dataset {
        name: "user_settings",
        query: "SELECT user_id, sub_country, phone_service_active, llm_provider, voice_provider
            FROM user_settings ORDER BY user_id, id",
        timestamp: None,
        description: "Current billing country and service/provider settings only; historical settings may differ.",
    },
    Dataset {
        name: "llm_usage",
        query: "SELECT id, user_id, provider, model, callsite, prompt_tokens, completion_tokens,
            total_tokens, cached_prompt_tokens, provider_cost_usd, customer_cost_usd,
            pricing_snapshot, created_at FROM llm_usage_logs ORDER BY id",
        timestamp: Some("created_at"),
        description: "One recorded AI call per row. Costs are saved USD values; older unpriced calls stay null. customer_cost_usd is a projected customer charge, not collected revenue. pricing_snapshot contains only typed pricing fields; snapshot_export_status reports missing/invalid snapshots.",
    },
    Dataset {
        name: "sms_usage",
        query: "SELECT id, user_id, message_sid, direction, status, created_at, updated_at,
            price, price_unit, fallback_provider, fallback_attempted_at, fallback_message_sid
            FROM message_status_log ORDER BY id",
        timestamp: Some("created_at"),
        description: "One retained message-status row, not an SMS segment. price is the signed provider amount in price_unit (Twilio charges are negative); null means unknown. No message bodies, phone numbers, or error text. Segment counts and original provider are not separately stored; fallback costs may be incomplete. Current BYOT setting cannot establish who paid a historical charge.",
    },
    Dataset {
        name: "legacy_usage",
        query: "SELECT id, user_id, sid, activity_type, credits, created_at, time_consumed,
            success, status, call_duration, recharge_threshold_timestamp, zero_credits_timestamp
            FROM usage_logs ORDER BY id",
        timestamp: Some("created_at"),
        description: "Legacy activity/credit ledger, including calls and SMS. credits are recorded customer usage deductions, not proof of provider cost or cash revenue. time_consumed and call_duration are stored seconds. Unknown activity labels become other_redacted because this column also stored free-form admin email subjects; activity_type_redacted marks those rows. These activities can overlap SMS, AI and billing rows; do not sum costs across ledgers blindly.",
    },
    Dataset {
        name: "billing_accounts",
        query: "SELECT user_id, metronome_customer_id, metronome_contract_id, overage_enabled,
            overage_consent_at, payment_ready, usage_entitled, provisioning_status,
            legacy_credit_migrated, legacy_overage_preference_migrated, created_at, updated_at
            FROM billing_accounts ORDER BY user_id",
        timestamp: Some("created_at"),
        description: "Current Metronome account state and reconciliation IDs. created_at is billing-account creation, not user signup. No payment credentials or provisioning error text.",
    },
    Dataset {
        name: "billing_usage",
        query: "SELECT transaction_id, user_id, event_type, cost_microusd, occurred_at, status,
            attempts, next_attempt_at, created_at, sent_at, provider_reconciled_at,
            provider_status, invoice_visible FROM billing_usage_events ORDER BY transaction_id",
        timestamp: Some("occurred_at"),
        description: "Customer usage submitted/queued to Metronome: cost_microusd / 1000000 = USD. This can be capped and can overlap other usage datasets. Submission or invoice visibility does not prove payment; these are not vendor costs. Includes pending/failed rows without last_error text.",
    },
    Dataset {
        name: "billing_usage_intents",
        query: "SELECT transaction_id, user_id, event_type, status, created_at, finalized_at
            FROM billing_usage_intents ORDER BY transaction_id",
        timestamp: Some("created_at"),
        description: "Usage lifecycle records; unresolved intents can indicate charges still awaiting provider reconciliation. Join billing_usage by transaction_id; do not count finalized intents as extra usage.",
    },
    Dataset {
        name: "billing_webhook_events",
        query: "SELECT event_id, event_type, received_at, status, attempts, processed_at
            FROM billing_webhook_events ORDER BY event_id",
        timestamp: Some("received_at"),
        description: "Billing webhook processing metadata only. No payloads, error text, payment amounts, or user linkage are stored here.",
    },
    Dataset {
        name: "refund_snapshot",
        query: "SELECT id, user_id, has_refunded, last_credit_pack_amount,
            last_credit_pack_purchase_timestamp, refunded_at FROM refund_info ORDER BY id",
        timestamp: Some("last_credit_pack_purchase_timestamp"),
        description: "Latest credit-pack/refund state only, not a complete payment/refund history. last_credit_pack_amount is a recorded credit-pack amount, not verified cash revenue or a refund amount.",
    },
    Dataset {
        name: "bridge_bandwidth",
        query: "SELECT id, user_id, bridge_type, direction, bytes_estimate, created_at
            FROM bridge_bandwidth_logs ORDER BY id",
        timestamp: Some("created_at"),
        description: "Per-event estimated bytes, not metered invoice amounts. No bridge room IDs or content.",
    },
    Dataset {
        name: "bridge_disconnections",
        query: "SELECT id, user_id, bridge_type, detected_at FROM bridge_disconnection_events ORDER BY id",
        timestamp: Some("detected_at"),
        description: "Recorded bridge disconnection events, without account handles or room IDs.",
    },
    Dataset {
        name: "processed_email_counts",
        query: "SELECT user_id, processed_at, COUNT(*) AS processed_count FROM processed_emails
            GROUP BY user_id, processed_at ORDER BY user_id, processed_at",
        timestamp: Some("processed_at"),
        description: "Email-processing counts at the original timestamp resolution; no mailbox IDs, email UIDs, senders, recipients, subjects or contents. Deduplication records may have been pruned.",
    },
    Dataset {
        name: "light_tool_devices",
        query: "SELECT id AS device_id, user_id, trial_started_at, trial_expires_at,
            trial_messages_used, last_seen_at, revoked_at, created_at, updated_at
            FROM light_tool_devices ORDER BY id",
        timestamp: Some("created_at"),
        description: "Device trial counters and lifecycle timestamps. No installation identifiers, tokens, or push endpoints. User linkage is the current linkage.",
    },
    Dataset {
        name: "light_tool_runs",
        query: "SELECT id, device_id, account_user_id, status, created_at, updated_at, completed_at
            FROM light_tool_runs ORDER BY id",
        timestamp: Some("created_at"),
        description: "Run counts/status/timing with account_user_id captured at run creation. No client-message IDs, prompts, images, responses, activity text or error text. Retained runs may overlap AI usage.",
    },
    Dataset {
        name: "agent_actions",
        query: "SELECT id, user_id, action_kind, outcome, created_at FROM agent_action_audit ORDER BY id",
        timestamp: Some("created_at"),
        description: "Action counts and outcomes; no credentials, arguments, contact details or content.",
    },
    Dataset {
        name: "ai_model_prices",
        query: "SELECT provider, model, rates, fetched_at FROM ai_model_prices ORDER BY provider, model",
        timestamp: Some("fetched_at"),
        description: "Latest saved rates in USD per million tokens and USD per request/search, not a historical rate ledger. Only typed pricing fields are exported; rates_export_status reports invalid data. Never apply these to old rows as if they were historical actual costs.",
    },
    Dataset {
        name: "country_prices",
        query: "SELECT country_code, has_local_numbers, outbound_sms_price, inbound_sms_price,
            outbound_voice_price_per_min, inbound_voice_price_per_min, last_checked, created_at
            FROM country_availability ORDER BY id",
        timestamp: Some("last_checked"),
        description: "Latest saved country price estimates, not historical invoices; the source table does not store a currency field.",
    },
    Dataset {
        name: "waitlist",
        query: "SELECT id, email, created_at FROM waitlist ORDER BY id",
        timestamp: Some("created_at"),
        description: "Waitlist signup email addresses and timestamps, separate from registered users.",
    },
];

#[derive(QueryableByName)]
struct JsonRow {
    #[diesel(sql_type = Text)]
    row_json: String,
}

#[derive(Serialize)]
struct DatasetSummary {
    name: &'static str,
    description: &'static str,
    timestamp_field: Option<&'static str>,
    row_count: u64,
    earliest_timestamp: Option<i64>,
    latest_timestamp: Option<i64>,
}

pub struct AnalyticsExportRepository {
    pool: PgDbPool,
}

impl AnalyticsExportRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    pub fn create_archive(&self) -> anyhow::Result<std::fs::File> {
        let mut conn = self.pool.get()?;
        // Anonymous temporary file, removed automatically, including on failure.
        // Finish the archive before sending HTTP headers so a DB failure cannot
        // masquerade as a successful partial export. No full archive in memory.
        let mut file = tempfile::tempfile()?;
        {
            let mut writer = BufWriter::new(&mut file);
            write_archive(&mut conn, &mut writer)?;
            writer.flush()?;
        }
        file.seek(SeekFrom::Start(0))?;
        Ok(file)
    }
}

/// Public for integration tests. Caller must discard output if this returns Err.
pub fn write_archive(conn: &mut PgConnection, writer: &mut impl Write) -> anyhow::Result<()> {
    conn.build_transaction().repeatable_read().read_only().run(|conn| {
        let started_at = chrono::Utc::now().timestamp();
        writer.write_all(b"{\"schema_version\":1,\"datasets\":{")?;
        let mut summaries = Vec::new();
        for (index, dataset) in DATASETS.iter().enumerate() {
            if index > 0 {
                writer.write_all(b",")?;
            }
            serde_json::to_writer(&mut *writer, dataset.name)?;
            writer.write_all(b":[")?;
            // SQL only comes from the static allowlist above. A server cursor
            // bounds memory without OFFSET scans or dropping older history.
            conn.batch_execute(&format!(
                "DECLARE analytics_export NO SCROLL CURSOR FOR SELECT row_to_json(export_row)::text AS row_json FROM ({}) export_row",
                dataset.query
            ))?;
            let mut summary = DatasetSummary {
                name: dataset.name,
                description: dataset.description,
                timestamp_field: dataset.timestamp,
                row_count: 0,
                earliest_timestamp: None,
                latest_timestamp: None,
            };
            loop {
                let rows = diesel::sql_query("FETCH FORWARD 1000 FROM analytics_export")
                    .load::<JsonRow>(conn)?;
                if rows.is_empty() {
                    break;
                }
                for row in rows {
                    let mut value: Value = serde_json::from_str(&row.row_json)?;
                    sanitize_pricing(dataset.name, &mut value);
                    if dataset.name == "legacy_usage" {
                        let known = value["activity_type"].as_str().is_some_and(known_activity_type);
                        value["activity_type_redacted"] = json!(!known);
                        if !known {
                            value["activity_type"] = json!("other_redacted");
                        }
                    }
                    if let Some(ts) = dataset.timestamp.and_then(|field| value[field].as_i64()) {
                        summary.earliest_timestamp = Some(summary.earliest_timestamp.map_or(ts, |old| old.min(ts)));
                        summary.latest_timestamp = Some(summary.latest_timestamp.map_or(ts, |old| old.max(ts)));
                    }
                    if summary.row_count > 0 {
                        writer.write_all(b",")?;
                    }
                    serde_json::to_writer(&mut *writer, &value)?;
                    summary.row_count += 1;
                }
            }
            conn.batch_execute("CLOSE analytics_export")?;
            writer.write_all(b"]")?;
            summaries.push(summary);
        }
        writer.write_all(b"},\"manifest\":")?;
        serde_json::to_writer(&mut *writer, &json!({
            "complete": true,
            "started_at": started_at,
            "finished_at": chrono::Utc::now().timestamp(),
            "consistency": "PostgreSQL REPEATABLE READ, READ ONLY; all retained rows visible in one snapshot",
            "timestamps": "Unix seconds, UTC; null means unknown, never zero-filled",
            "datasets": summaries,
            "privacy": "Explicit allowlist of analytics and registered/waitlist email addresses. No messages (including encrypted bodies), prompts, responses, transcripts, images, phone numbers, contacts, credentials, arbitrary metadata, or error text.",
            "limitations": [
                "Only data still retained in this database is available. Deleted/pruned/unlogged history cannot be reconstructed; orphan usage rows are preserved without an email lookup.",
                "No complete cash-revenue, subscription-change or refund ledger is stored locally. Export Stripe payments, invoices, refunds and fees, plus Metronome invoices separately before closing those accounts. Reconcile using the exported customer IDs.",
                "Hosting, number rental, provider minimums, taxes, currency conversions and other infrastructure/vendor costs are not a complete local ledger. Preserve provider invoices separately.",
                "Provider cost, projected customer charge, legacy credits and submitted billing usage are distinct measures. They overlap and must not be added as independent costs or treated as collected revenue.",
                "SMS price signs and currencies are preserved; no conversion, segment estimation or historical repricing is performed. Null costs are unknown, not free usage.",
                "User plans, BYOT status, country, balances and catalog prices are current snapshots, not historical values.",
                "Run again after final usage and delayed provider callbacks/reconciliation have settled. A successful export does not stop live traffic."
            ]
        }))?;
        writer.write_all(b"}")?;
        Ok(())
    })
}

// This column is also used for arbitrary admin alert subjects. Preserve only
// known application categories and their bounded delivery suffixes, never a
// generic character/length regex that could admit private text.
fn known_activity_type(value: &str) -> bool {
    const BASES: &[&str] = &[
        "sms",
        "sms_test",
        "message",
        "call",
        "voice",
        "web_chat",
        "web_voice",
        "noti_msg",
        "noti_call",
        "webhook_sms",
        "rule",
        "rule_test",
        "digest",
        "event_notification",
        "tracked_item_update",
        "system_screened",
        "system_important",
        "system_critical",
        "accountability_friend",
        "system_alert_feedback_worth_it",
        "system_alert_feedback_should_wait",
        "whatsapp_native_activity_reminder",
        "whatsapp_profile",
        "telegram_profile",
        "signal_profile",
        "email_profile",
        "tesla_profile",
        "whatsapp_critical",
        "telegram_critical",
        "signal_critical",
        "email_critical",
        "tesla_critical",
        "tesla_ready_to_drive",
        "tesla_ready_timeout",
        "tesla_climate_stopped",
        "tesla_charging_complete",
    ];
    let mut base = value;
    for _ in 0..=2 {
        if BASES.contains(&base) {
            return true;
        }
        match ["_call_conditional", "_light_phone", "_sms", "_call"]
            .iter()
            .find_map(|suffix| base.strip_suffix(suffix))
        {
            Some(stripped) => base = stripped,
            None => return false,
        }
    }
    false
}

// Pricing blobs are the only structured text columns allowed through the query.
// Deserialize and reserialize a known type, dropping all unrecognized fields.
// Never fall back to copying a malformed/raw JSON string into the export.
fn sanitize_pricing(dataset: &str, row: &mut Value) {
    let (field, sanitized) = match dataset {
        "llm_usage" => (
            "pricing_snapshot",
            row["pricing_snapshot"]
                .as_str()
                .and_then(|text| serde_json::from_str::<CostSnapshot>(text).ok())
                .and_then(|mut snapshot| {
                    snapshot.quote.source = match snapshot.quote.source.as_str() {
                        "provider_api" | "saved_api" | "default_model" | "default_provider" => {
                            snapshot.quote.source
                        }
                        _ => "unknown".into(),
                    };
                    serde_json::to_value(snapshot).ok()
                }),
        ),
        "ai_model_prices" => (
            "rates",
            row["rates"]
                .as_str()
                .and_then(|text| serde_json::from_str::<ModelRates>(text).ok())
                .and_then(|rates| serde_json::to_value(rates).ok()),
        ),
        _ => return,
    };
    let status = if row[field].is_null() {
        "missing"
    } else if sanitized.is_some() {
        "exported"
    } else {
        "invalid"
    };
    let status_field = if dataset == "llm_usage" {
        "snapshot_export_status"
    } else {
        "rates_export_status"
    };
    row[status_field] = json!(status);
    row[field] = sanitized.unwrap_or(Value::Null);
}
