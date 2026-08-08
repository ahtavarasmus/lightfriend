use axum::http::StatusCode;
use chrono::{Offset, TimeZone};
use openai_api_rs::v1::{chat_completion, types};
use serde::Deserialize;
use std::collections::HashMap;

use crate::api::twilio_sms::TwilioResponse;
use crate::repositories::temporary_alert_suppressions_repository::{
    topic_matches, KIND_QUIET, KIND_TOPIC, SCOPE_ALL, SCOPE_CRITICAL, SCOPE_DIGEST,
};
use crate::tools::registry::{write_outgoing_history, ToolContext, ToolHandler, ToolResult};
use crate::TemporaryAlertSuppressionsRepository;

pub struct ManageAlertSuppressionHandler;

#[derive(Deserialize)]
struct ManageAlertSuppressionArgs {
    action: String,
    #[serde(default)]
    topic: Option<String>,
    #[serde(default)]
    quiet_scope: Option<String>,
    #[serde(default)]
    duration_minutes: Option<i64>,
    #[serde(default)]
    until: Option<String>,
}

fn direct(ctx: &ToolContext<'_>, message: String) -> ToolResult {
    write_outgoing_history(
        ctx.state,
        ctx.user_id,
        "manage_alert_suppression",
        &ctx.tool_call_id,
        &message,
        ctx.current_time,
    );
    ToolResult::EarlyReturn {
        response: TwilioResponse {
            message,
            created_item_id: None,
        },
        status: StatusCode::OK,
    }
}

fn user_timezone(ctx: &ToolContext<'_>) -> Result<String, String> {
    crate::proactive::utils::reminder_timezone(
        ctx.state,
        ctx.user_id,
        chrono::Utc::now().timestamp() as i32,
    )
}

fn resolve_expiry(
    duration_minutes: Option<i64>,
    until: Option<&str>,
    timezone: &str,
) -> Result<i32, String> {
    if duration_minutes.is_some() && until.is_some() {
        return Err("Give me either a duration or an end time, not both.".to_string());
    }
    let now = chrono::Utc::now();
    let expiry = if let Some(minutes) = duration_minutes {
        if !(1..=43_200).contains(&minutes) {
            return Err(
                "How long should this last? Choose between 1 minute and 30 days.".to_string(),
            );
        }
        now + chrono::Duration::minutes(minutes)
    } else if let Some(until) = until {
        let timestamp = crate::proactive::utils::parse_reminder_time_in_zone(
            until,
            timezone,
            now.timestamp() as i32,
        )?;
        chrono::DateTime::from_timestamp(timestamp as i64, 0)
            .ok_or_else(|| "That end time is out of range.".to_string())?
    } else {
        return Err("How long should I keep this temporary setting?".to_string());
    };
    if expiry <= now {
        return Err("That end time has already passed. When should this setting end?".to_string());
    }
    if expiry > now + chrono::Duration::days(30) {
        return Err(
            "Temporary alert settings can last up to 30 days. What shorter duration should I use?"
                .to_string(),
        );
    }
    i32::try_from(expiry.timestamp()).map_err(|_| "That end time is out of range.".to_string())
}

pub fn format_persisted_time(timestamp: i32, timezone: &str) -> String {
    let tz = timezone.parse::<chrono_tz::Tz>().unwrap_or(chrono_tz::UTC);
    let local = chrono::Utc
        .timestamp_opt(timestamp as i64, 0)
        .single()
        .expect("valid i32 epoch")
        .with_timezone(&tz);
    let offset = local.offset().fix().local_minus_utc();
    let sign = if offset >= 0 { '+' } else { '-' };
    let absolute = offset.abs();
    format!(
        "{} ({} UTC{}{:02}:{:02})",
        local.format("%Y-%m-%d %H:%M %Z"),
        timezone,
        sign,
        absolute / 3600,
        (absolute % 3600) / 60
    )
}

fn quiet_scope_label(scope: &str) -> &'static str {
    match scope {
        SCOPE_CRITICAL => "calls and critical alerts",
        SCOPE_DIGEST => "digests",
        _ => "all alerts",
    }
}

pub fn persisted_suppression_confirmation(
    persisted: &crate::models::user_models::TemporaryAlertSuppression,
) -> String {
    if persisted.kind == KIND_TOPIC {
        format!(
            "Ignoring matching '{}' alerts across connected sources until {}.",
            persisted.match_text.as_deref().unwrap_or("topic"),
            format_persisted_time(persisted.expires_at, &persisted.timezone)
        )
    } else {
        format!(
            "Quiet Mode persisted for {} until {}.",
            quiet_scope_label(&persisted.scope),
            format_persisted_time(persisted.expires_at, &persisted.timezone)
        )
    }
}

#[async_trait::async_trait]
impl ToolHandler for ManageAlertSuppressionHandler {
    fn name(&self) -> &'static str {
        "manage_alert_suppression"
    }

    fn definition(&self) -> chat_completion::Tool {
        let mut properties = HashMap::new();
        properties.insert(
            "action".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some("ignore_expected creates a semantic temporary ignore window; start_quiet starts Quiet Mode; end_quiet ends Quiet Mode early.".to_string()),
                enum_values: Some(vec!["ignore_expected".into(), "start_quiet".into(), "end_quiet".into()]),
                ..Default::default()
            }),
        );
        properties.insert(
            "topic".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some("Required for ignore_expected. A concise semantic cross-channel scope such as 'Coinbase transactions'. Do not include a channel name unless the user explicitly makes it part of the topic.".to_string()),
                ..Default::default()
            }),
        );
        properties.insert(
            "quiet_scope".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "Quiet Mode category. Omit to default to all alerts.".to_string(),
                ),
                enum_values: Some(vec![
                    SCOPE_ALL.into(),
                    SCOPE_CRITICAL.into(),
                    SCOPE_DIGEST.into(),
                ]),
                ..Default::default()
            }),
        );
        properties.insert(
            "duration_minutes".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::Number),
                description: Some("Duration in minutes. Use for requests such as 'for two hours' or 'for three days'.".to_string()),
                ..Default::default()
            }),
        );
        properties.insert(
            "until".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some("Exact end time as ISO 8601, preferably with UTC offset. Use for 'until tomorrow morning'. Never invent an ambiguous time.".to_string()),
                ..Default::default()
            }),
        );
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: self.name().to_string(),
                description: Some("Create or end temporary notification suppression. Use for 'ignore Coinbase transactions for two hours', 'be quiet for three days', 'do not disturb me until tomorrow morning', and explicit requests to re-enable alerts. Underlying messages are always retained. The tool returns the exact persisted scope and expiry directly; do not add a second confirmation.".to_string()),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(properties),
                    required: Some(vec!["action".to_string()]),
                },
            },
        }
    }

    fn is_restricted(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        let args: ManageAlertSuppressionArgs = serde_json::from_str(ctx.arguments)
            .map_err(|error| format!("Invalid alert suppression arguments: {}", error))?;
        let repository = TemporaryAlertSuppressionsRepository::new(ctx.state.pg_pool.clone());

        if args.action == "end_quiet" {
            let scope = args.quiet_scope.as_deref();
            let ended = repository
                .end_quiet(ctx.user_id, scope)
                .map_err(|error| format!("Failed to end Quiet Mode: {}", error))?;
            let message = match ended.first().and_then(|row| row.ended_at) {
                Some(ended_at) => {
                    let mut ended_scopes: Vec<&str> = ended
                        .iter()
                        .map(|row| quiet_scope_label(&row.scope))
                        .collect();
                    ended_scopes.sort_unstable();
                    ended_scopes.dedup();
                    format!(
                        "Quiet Mode ended for {} at {}.",
                        ended_scopes.join(", "),
                        format_persisted_time(ended_at, &ended[0].timezone)
                    )
                }
                None => "Quiet Mode was already off for that scope.".to_string(),
            };
            return Ok(direct(&ctx, message));
        }

        let timezone = match user_timezone(&ctx) {
            Ok(timezone) => timezone,
            Err(question) => return Ok(direct(&ctx, question)),
        };

        let expires_at =
            match resolve_expiry(args.duration_minutes, args.until.as_deref(), &timezone) {
                Ok(expires_at) => expires_at,
                Err(question) => return Ok(direct(&ctx, question)),
            };

        let (kind, scope, match_text) = match args.action.as_str() {
            "ignore_expected" => {
                let topic = args.topic.as_deref().map(str::trim).unwrap_or("");
                if topic.is_empty() || !topic_matches(topic, topic) {
                    return Ok(direct(
                        &ctx,
                        "What specific sender or topic should I ignore temporarily?".to_string(),
                    ));
                }
                (KIND_TOPIC, SCOPE_ALL, Some(topic))
            }
            "start_quiet" => (
                KIND_QUIET,
                match args.quiet_scope.as_deref().unwrap_or(SCOPE_ALL) {
                    SCOPE_ALL => SCOPE_ALL,
                    SCOPE_CRITICAL => SCOPE_CRITICAL,
                    SCOPE_DIGEST => SCOPE_DIGEST,
                    _ => {
                        return Ok(direct(
                            &ctx,
                            "Should Quiet Mode cover all alerts, calls and critical alerts, or digests?"
                                .to_string(),
                        ))
                    }
                },
                None,
            ),
            _ => {
                return Ok(direct(
                    &ctx,
                    "Should I ignore a topic, start Quiet Mode, or end Quiet Mode?".to_string(),
                ))
            }
        };

        let persisted = repository
            .create(ctx.user_id, kind, scope, match_text, &timezone, expires_at)
            .map_err(|error| format!("Failed to persist alert suppression: {}", error))?;
        let confirmation = persisted_suppression_confirmation(&persisted);
        Ok(direct(&ctx, confirmation))
    }
}
