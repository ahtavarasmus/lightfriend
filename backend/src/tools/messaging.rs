use axum::http::StatusCode;
use openai_api_rs::v1::chat_completion;
use serde::Deserialize;
use std::collections::HashMap;

use crate::api::twilio_sms::TwilioResponse;
use crate::models::ontology_models::{OntChannel, PersonWithChannels};
use crate::tools::registry::{
    write_outgoing_error_history, write_outgoing_history, ToolContext, ToolHandler, ToolResult,
};
use crate::AppState;
use std::sync::Arc;

// ─── send_chat_message (outgoing) ────────────────────────────────────────────

pub struct SendMessageHandler;

#[async_trait::async_trait]
impl ToolHandler for SendMessageHandler {
    fn name(&self) -> &'static str {
        "send_chat_message"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::bridge::get_send_chat_message_tool()
    }

    fn is_outgoing(&self) -> bool {
        true
    }

    fn is_restricted(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::info!(
            "SEND_FLOW send_chat_message tool execute() called for user={}, args={}",
            ctx.user_id,
            ctx.arguments
        );
        match crate::tool_call_utils::bridge::handle_send_chat_message(
            ctx.state,
            ctx.user_id,
            ctx.arguments,
            ctx.user,
            ctx.image_url,
            ctx.skip_sms,
        )
        .await
        {
            Ok((status, _headers, axum::Json(twilio_response))) => {
                tracing::info!(
                    "SEND_FLOW handle_send_chat_message returned OK for user={}, status={}, msg={}",
                    ctx.user_id,
                    status,
                    twilio_response.message
                );
                write_outgoing_history(
                    ctx.state,
                    ctx.user_id,
                    "send_chat_message",
                    &ctx.tool_call_id,
                    &twilio_response.message,
                    ctx.current_time,
                );
                tracing::info!(
                    "SEND_FLOW Returning EarlyReturn for user={}, this should spawn delayed task and return immediately",
                    ctx.user_id
                );
                Ok(ToolResult::EarlyReturn {
                    response: twilio_response,
                    status,
                })
            }
            Err(e) => {
                tracing::error!(
                    "SEND_FLOW handle_send_chat_message FAILED for user={}: {}",
                    ctx.user_id,
                    e
                );
                write_outgoing_error_history(
                    ctx.state,
                    ctx.user_id,
                    "send_chat_message",
                    &ctx.tool_call_id,
                    "Failed to send chat message",
                    ctx.current_time,
                );
                Ok(ToolResult::EarlyReturn {
                    response: TwilioResponse {
                        message: "Failed to process chat message request".to_string(),
                        created_item_id: None,
                    },
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                })
            }
        }
    }
}

// ─── wait_for_reply (one-shot incoming alert) ───────────────────────────────

pub struct WaitForReplyHandler;

#[derive(Deserialize)]
struct WaitForReplyArgs {
    contact: String,
    #[serde(default)]
    platform: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyWatchTarget {
    pub display_name: String,
    pub platform: String,
    pub room_id: String,
    pub contact_identifier: String,
}

fn is_supported_reply_platform(platform: &str) -> bool {
    matches!(platform, "whatsapp" | "telegram" | "signal")
}

fn platform_label(platform: &str) -> String {
    if platform.eq_ignore_ascii_case("whatsapp") {
        return "WhatsApp".to_string();
    }
    let mut chars = platform.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => platform.to_string(),
    }
}

/// Resolve a user-supplied contact without guessing between people or chats.
/// Exact base-name/nickname matches win; otherwise a unique substring match is
/// accepted. Multiple people or multiple eligible channels return a concise
/// clarification message for the conversational agent to pass through.
pub fn resolve_reply_watch_target(
    persons: &[PersonWithChannels],
    contact: &str,
    platform: Option<&str>,
) -> Result<ReplyWatchTarget, String> {
    let contact = contact.trim();
    if contact.is_empty() {
        return Err("Who should I watch for a reply from?".to_string());
    }

    let requested_platform = platform.map(|p| p.trim().to_lowercase());
    if let Some(ref platform) = requested_platform {
        if !is_supported_reply_platform(platform) {
            return Err(format!(
                "I can watch replies on WhatsApp, Telegram, or Signal, not {}.",
                platform
            ));
        }
    }

    let needle = contact.to_lowercase();
    let exact: Vec<&PersonWithChannels> = persons
        .iter()
        .filter(|person| {
            person.display_name().eq_ignore_ascii_case(contact)
                || person.person.name.eq_ignore_ascii_case(contact)
        })
        .collect();
    let matches = if exact.is_empty() {
        persons
            .iter()
            .filter(|person| {
                person.display_name().to_lowercase().contains(&needle)
                    || person.person.name.to_lowercase().contains(&needle)
            })
            .collect::<Vec<_>>()
    } else {
        exact
    };

    if matches.is_empty() {
        return Err(format!("I couldn't find a contact matching '{}'.", contact));
    }
    if matches.len() > 1 {
        let choices = matches
            .iter()
            .map(|person| person.display_name())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!("Which contact did you mean: {}?", choices));
    }

    let person = matches[0];
    let eligible_channels: Vec<&OntChannel> = person
        .channels
        .iter()
        .filter(|channel| {
            is_supported_reply_platform(&channel.platform)
                && channel.room_id.is_some()
                && requested_platform
                    .as_deref()
                    .map(|platform| platform == channel.platform)
                    .unwrap_or(true)
        })
        .collect();

    if eligible_channels.is_empty() {
        return if let Some(platform) = requested_platform {
            Err(format!(
                "I couldn't find a synced {} conversation for {}.",
                platform,
                person.display_name()
            ))
        } else {
            Err(format!(
                "I couldn't find a synced WhatsApp, Telegram, or Signal conversation for {}.",
                person.display_name()
            ))
        };
    }
    if eligible_channels.len() > 1 {
        let choices = eligible_channels
            .iter()
            .map(|channel| platform_label(&channel.platform))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "Which conversation should I watch for {}: {}?",
            person.display_name(),
            choices
        ));
    }

    let channel = eligible_channels[0];
    Ok(ReplyWatchTarget {
        display_name: person.display_name().to_string(),
        platform: channel.platform.clone(),
        room_id: channel.room_id.clone().expect("filtered to room_id"),
        contact_identifier: channel
            .handle
            .clone()
            .unwrap_or_else(|| person.display_name().to_string()),
    })
}

/// Arm a standalone one-shot watch from an already-authorized user command.
pub fn arm_wait_for_reply(
    state: &Arc<AppState>,
    user_id: i32,
    contact: &str,
    platform: Option<&str>,
) -> String {
    let persons = match state
        .ontology_repository
        .get_persons_with_channels(user_id, 500, 0)
    {
        Ok(persons) => persons,
        Err(error) => {
            tracing::warn!(
                "REPLY_WATCH failed to load contacts user={}: {}",
                user_id,
                error
            );
            return "I couldn't load your contacts right now. Please try again.".to_string();
        }
    };
    let target = match resolve_reply_watch_target(&persons, contact, platform) {
        Ok(target) => target,
        Err(clarification) => return clarification,
    };

    match state.pending_reply_watches_repository.arm_bridge(
        user_id,
        &target.room_id,
        &target.contact_identifier,
        &target.display_name,
    ) {
        Ok(_) => format!(
            "Watching for {}'s next {} reply for 24 hours.",
            target.display_name,
            platform_label(&target.platform)
        ),
        Err(error) => {
            tracing::warn!(
                "REPLY_WATCH failed to arm standalone watch user={}: {}",
                user_id,
                error
            );
            "I couldn't start that reply watch. Please try again.".to_string()
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for WaitForReplyHandler {
    fn name(&self) -> &'static str {
        "wait_for_reply"
    }

    fn definition(&self) -> chat_completion::Tool {
        use openai_api_rs::v1::types;

        let mut properties = HashMap::new();
        properties.insert(
            "contact".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "The contact/person whose next reply the user wants to be alerted about."
                        .to_string(),
                ),
                ..Default::default()
            }),
        );
        properties.insert(
            "platform".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "Optional conversation platform. Omit when the contact has only one synced conversation; use the user's clarification when they specify one."
                        .to_string(),
                ),
                enum_values: Some(vec![
                    "whatsapp".to_string(),
                    "telegram".to_string(),
                    "signal".to_string(),
                ]),
                ..Default::default()
            }),
        );
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: self.name().to_string(),
                description: Some(
                    "Create a temporary one-shot alert for the next incoming reply from a contact or conversation. Use when the user says things like 'let me know when Anna replies' or 'tell me when I hear back from Sam'. This does not send a message and does not create a permanent rule. If the contact or conversation is ambiguous, the tool returns a clarification question instead of guessing. The watch expires after 24 hours and clears after the first matching reply."
                        .to_string(),
                ),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(properties),
                    required: Some(vec!["contact".to_string()]),
                },
            },
        }
    }

    fn is_restricted(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        let args: WaitForReplyArgs = serde_json::from_str(ctx.arguments)
            .map_err(|error| format!("Invalid wait_for_reply arguments: {}", error))?;
        Ok(ToolResult::Answer(arm_wait_for_reply(
            ctx.state,
            ctx.user_id,
            &args.contact,
            args.platform.as_deref(),
        )))
    }
}
