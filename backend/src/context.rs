//! Centralized context assembly for LLM calls.
//!
//! `ContextBuilder` assembles what the agent needs before any LLM call.
//! Bare `.build()` gives just client/provider/model (cheap - 1 DB query).
//! Chain `.with_user_context()`, `.with_tools()`, `.with_history()` to
//! opt into heavier layers.

use std::sync::Arc;

use chrono::{FixedOffset, Offset, TimeZone, Utc};
use chrono_tz::Tz;
use openai_api_rs::v1::api::OpenAIClient;
use openai_api_rs::v1::chat_completion::{self, ChatCompletionMessage};

use crate::models::user_models::{User, UserSettings};
use crate::pg_models::PgUserInfo;
use crate::{AiProvider, AppState, ModelPurpose, UserCoreOps};

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("user not found: {0}")]
    UserNotFound(String),

    #[error("database error: {0}")]
    Database(#[from] diesel::result::Error),

    #[error("timezone error: {0}")]
    Timezone(String),

    #[error("AI client error: {0}")]
    AiClient(String),
}

// ---------------------------------------------------------------------------
// TimezoneInfo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TimezoneInfo {
    pub tz_str: String,
    pub offset_hours: i32,
    pub offset_minutes: i32,
    pub offset_string: String,
    pub formatted_now: String,
    pub fixed_offset: FixedOffset,
}

impl TimezoneInfo {
    fn compute(tz_str: &str) -> Self {
        let (hours, minutes) = match tz_str.parse::<Tz>() {
            Ok(tz) => {
                let now = Utc::now().with_timezone(&tz);
                let total_seconds = now.offset().fix().local_minus_utc();
                (total_seconds / 3600, (total_seconds.abs() % 3600) / 60)
            }
            Err(_) => {
                tracing::error!("Failed to parse timezone {}, defaulting to UTC", tz_str);
                (0, 0)
            }
        };

        let offset_seconds = hours * 3600 + minutes * 60 * if hours >= 0 { 1 } else { -1 };
        let fixed_offset = FixedOffset::east_opt(offset_seconds)
            .unwrap_or_else(|| FixedOffset::east_opt(0).unwrap());

        let formatted_now = Utc::now().with_timezone(&fixed_offset).to_rfc3339();

        let offset_string = format!(
            "{}{:02}:{:02}",
            if hours >= 0 { "+" } else { "-" },
            hours.abs(),
            minutes.abs()
        );

        Self {
            tz_str: tz_str.to_string(),
            offset_hours: hours,
            offset_minutes: minutes,
            offset_string,
            formatted_now,
            fixed_offset,
        }
    }
}

// ---------------------------------------------------------------------------
// AgentContext
// ---------------------------------------------------------------------------

pub struct AgentContext {
    // Always present (bare .build())
    pub state: Arc<AppState>,
    pub user: User,
    pub user_id: i32,
    pub provider: AiProvider,
    pub client: OpenAIClient,
    pub model: String,
    pub current_time_unix: i32,

    // Opt-in via .with_user_context()
    pub user_settings: Option<UserSettings>,
    pub user_info: Option<PgUserInfo>,
    pub timezone: Option<TimezoneInfo>,
    pub persons: Option<Vec<crate::models::ontology_models::PersonWithChannels>>,
    pub user_given_info: Option<String>,
    pub contacts_prompt_fragment: Option<String>,
    pub events_prompt_fragment: Option<String>,

    // Opt-in via .with_tools() / .with_mcp_tools()
    pub tools: Option<Vec<chat_completion::Tool>>,

    // Opt-in via .with_history()
    pub conversation_history: Option<Vec<ChatCompletionMessage>>,
}

// ---------------------------------------------------------------------------
// ContextBuilder
// ---------------------------------------------------------------------------

pub struct ContextBuilder {
    state: Arc<AppState>,
    user: Option<User>,
    user_id: Option<i32>,
    phone: Option<String>,
    want_user_context: bool,
    want_tools: bool,
    want_mcp_tools: bool,
    want_history: bool,
    history_depth_override: Option<i32>,
}

impl ContextBuilder {
    pub fn for_user(state: &Arc<AppState>, user_id: i32) -> Self {
        Self {
            state: Arc::clone(state),
            user: None,
            user_id: Some(user_id),
            phone: None,
            want_user_context: false,
            want_tools: false,
            want_mcp_tools: false,
            want_history: false,
            history_depth_override: None,
        }
    }

    pub fn for_phone(state: &Arc<AppState>, phone: &str) -> Self {
        Self {
            state: Arc::clone(state),
            user: None,
            user_id: None,
            phone: Some(phone.to_string()),
            want_user_context: false,
            want_tools: false,
            want_mcp_tools: false,
            want_history: false,
            history_depth_override: None,
        }
    }

    pub fn for_resolved_user(state: &Arc<AppState>, user: User) -> Self {
        let user_id = user.id;
        Self {
            state: Arc::clone(state),
            user: Some(user),
            user_id: Some(user_id),
            phone: None,
            want_user_context: false,
            want_tools: false,
            want_mcp_tools: false,
            want_history: false,
            history_depth_override: None,
        }
    }

    /// Fetch user settings, info, timezone, contacts.
    pub fn with_user_context(mut self) -> Self {
        self.want_user_context = true;
        self
    }

    pub fn with_tools(mut self) -> Self {
        self.want_tools = true;
        self
    }

    pub fn with_mcp_tools(mut self) -> Self {
        self.want_mcp_tools = true;
        self
    }

    /// Load conversation history. Implies `with_user_context()` (needs save_context).
    pub fn with_history(mut self) -> Self {
        self.want_history = true;
        self.want_user_context = true;
        self
    }

    pub fn with_history_depth(mut self, depth: i32) -> Self {
        self.want_history = true;
        self.history_depth_override = Some(depth);
        self
    }

    pub async fn build(self) -> Result<AgentContext, ContextError> {
        // 1. Resolve user
        let user = if let Some(u) = self.user {
            u
        } else if let Some(phone) = &self.phone {
            self.state
                .user_core
                .find_by_phone_number(phone)?
                .ok_or_else(|| ContextError::UserNotFound(phone.clone()))?
        } else if let Some(uid) = self.user_id {
            self.state
                .user_core
                .find_by_id(uid)?
                .ok_or_else(|| ContextError::UserNotFound(format!("id={}", uid)))?
        } else {
            return Err(ContextError::UserNotFound(
                "no user identifier provided".to_string(),
            ));
        };
        let user_id = user.id;

        // 2. LLM provider + client + model (always)
        let llm_pref = self
            .state
            .user_core
            .get_llm_provider(user_id)
            .unwrap_or(None);
        let provider = self
            .state
            .ai_config
            .provider_for_user_with_preference(llm_pref.as_deref());
        let client = self
            .state
            .ai_config
            .create_client(provider)
            .map_err(|e| ContextError::AiClient(e.to_string()))?;
        let model = self
            .state
            .ai_config
            .model(provider, ModelPurpose::Default)
            .to_string();

        let current_time_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i32;

        // 3. User context (opt-in)
        let (
            user_settings,
            user_info,
            timezone,
            persons,
            user_given_info,
            contacts_prompt_fragment,
            events_prompt_fragment,
        ) = if self.want_user_context {
            let settings = self.state.user_core.get_user_settings(user_id)?;
            let info = self.state.user_core.get_user_info(user_id)?;

            let tz_str = info.timezone.as_deref().unwrap_or("UTC");
            let tz = TimezoneInfo::compute(tz_str);

            let persons = self
                .state
                .ontology_repository
                .get_persons_with_channels(user_id, 500, 0)
                .ok();

            let fragment = if let Some(ref persons_list) = persons {
                if !persons_list.is_empty() {
                    let mut f = String::from("\n\nYour contacts:\n");
                    for p in persons_list {
                        let name = p.display_name();
                        let platforms: Vec<&str> =
                            p.channels.iter().map(|c| c.platform.as_str()).collect();
                        f.push_str(&format!("- {} ({})\n", name, platforms.join(", ")));
                    }
                    f
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            // Fetch the 5 most upcoming active events for the prompt
            let now_ts = current_time_unix;
            let events_fragment = match self
                .state
                .ontology_repository
                .get_upcoming_events(user_id, 5)
            {
                Ok(events) if !events.is_empty() => {
                    let mut f = String::from("\n\nYour active reminders/events:\n");
                    for e in &events {
                        let deadline_str = match e.due_at {
                            Some(ts) => {
                                let remaining = ts - now_ts;
                                let days = remaining / 86400;
                                if remaining < 0 {
                                    " [OVERDUE]".to_string()
                                } else if days == 0 {
                                    let hours = (remaining / 3600).max(1);
                                    format!(" [due in {} hours]", hours)
                                } else {
                                    format!(" [due in {} days]", days)
                                }
                            }
                            None => String::new(),
                        };
                        let desc: String = e.description.chars().take(200).collect();
                        f.push_str(&format!("- [id={}]{} {}\n", e.id, deadline_str, desc));
                    }
                    f
                }
                _ => String::new(),
            };

            let given_info = info.info.clone().unwrap_or_default();

            (
                Some(settings),
                Some(info),
                Some(tz),
                persons,
                Some(given_info),
                Some(fragment),
                Some(events_fragment),
            )
        } else {
            (None, None, None, None, None, None, None)
        };

        // 4. Tools (opt-in)
        let tools = if self.want_tools {
            let mut tool_defs = self
                .state
                .tool_registry
                .definitions_for_user(&self.state, user_id);
            if self.want_mcp_tools {
                let mcp =
                    crate::tool_call_utils::mcp::get_mcp_tools_for_user(&self.state, user_id).await;
                tool_defs.extend(mcp);
            }
            Some(tool_defs)
        } else {
            None
        };

        // 5. History (opt-in, requires user_settings for save_context)
        let conversation_history = if self.want_history {
            let depth = self.history_depth_override.unwrap_or_else(|| {
                user_settings
                    .as_ref()
                    .and_then(|s| s.save_context)
                    .unwrap_or(0)
            });
            if depth > 0 {
                let raw = self
                    .state
                    .user_repository
                    .get_conversation_history(user_id, depth as i64, true)
                    .unwrap_or_default();
                let tz_offset = timezone
                    .as_ref()
                    .map(|tz| tz.fixed_offset)
                    .unwrap_or_else(|| FixedOffset::east_opt(0).unwrap());
                Some(convert_history(raw, &tz_offset))
            } else {
                Some(Vec::new())
            }
        } else {
            None
        };

        Ok(AgentContext {
            state: self.state,
            user,
            user_id,
            provider,
            client,
            model,
            current_time_unix,
            user_settings,
            user_info,
            timezone,
            persons,
            user_given_info,
            contacts_prompt_fragment,
            events_prompt_fragment,
            tools,
            conversation_history,
        })
    }
}

// ---------------------------------------------------------------------------
// History conversion
// ---------------------------------------------------------------------------

const SKIP_FROM_HISTORY: &[&str] = &["get_weather"];

/// Minimum gap (seconds) before inserting a conversation break marker.
const GAP_THRESHOLD_SECS: i32 = 30 * 60; // 30 minutes

pub fn convert_history(
    raw: Vec<crate::pg_models::PgMessageHistory>,
    offset: &FixedOffset,
) -> Vec<ChatCompletionMessage> {
    let mut out = Vec::new();
    let mut last_ts: Option<i32> = None;

    for msg in raw.into_iter().rev() {
        // Before user messages, check for a significant time gap
        if msg.role == "user" && !msg.encrypted_content.trim().is_empty() {
            if let Some(prev) = last_ts {
                let gap = msg.created_at - prev;
                if gap >= GAP_THRESHOLD_SECS {
                    out.push(ChatCompletionMessage {
                        role: chat_completion::MessageRole::system,
                        content: chat_completion::Content::Text(format!(
                            "[{} gap - treat what follows as a new conversation]",
                            format_duration(gap)
                        )),
                        name: None,
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
            }
        }

        last_ts = Some(msg.created_at);

        if msg.role == "assistant" && msg.tool_calls_json.is_some() {
            continue;
        }

        if msg.role == "tool" {
            if let Some(ref tool_name) = msg.tool_name {
                if SKIP_FROM_HISTORY.contains(&tool_name.as_str()) {
                    continue;
                }
            }

            let context_content = msg.encrypted_content.clone();
            out.push(ChatCompletionMessage {
                role: chat_completion::MessageRole::assistant,
                content: chat_completion::Content::Text(context_content),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            });
            continue;
        }

        let role = match msg.role.as_str() {
            "user" => chat_completion::MessageRole::user,
            "assistant" => chat_completion::MessageRole::assistant,
            _ => continue,
        };

        if msg.encrypted_content.trim().is_empty() {
            continue;
        }

        // Prefix user messages with their timestamp
        let content = if role == chat_completion::MessageRole::user {
            let ts_str = offset
                .timestamp_opt(msg.created_at as i64, 0)
                .single()
                .map(|dt| dt.format("%-I:%M %p").to_string())
                .unwrap_or_default();
            format!("[{}] {}", ts_str, msg.encrypted_content)
        } else {
            msg.encrypted_content
        };

        out.push(ChatCompletionMessage {
            role,
            content: chat_completion::Content::Text(content),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        });
    }

    out
}

fn format_duration(secs: i32) -> String {
    if secs >= 86400 {
        let days = secs / 86400;
        if days == 1 {
            "1 day".to_string()
        } else {
            format!("{} days", days)
        }
    } else if secs >= 3600 {
        let hours = secs / 3600;
        if hours == 1 {
            "1 hour".to_string()
        } else {
            format!("{} hours", hours)
        }
    } else {
        let mins = secs / 60;
        format!("{} min", mins)
    }
}
