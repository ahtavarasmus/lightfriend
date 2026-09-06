//! Preserve usage metadata that the OpenAI-compatible SDK otherwise discards.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize)]
pub struct ChatResponse {
    pub id: Option<String>,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<openai_api_rs::v1::chat_completion::ChatCompletionChoice>,
    #[serde(default)]
    pub usage: TokenUsage,
    pub system_fingerprint: Option<String>,
    pub headers: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct TokenUsage {
    #[serde(default)]
    pub prompt_tokens: i32,
    #[serde(default)]
    pub completion_tokens: i32,
    #[serde(default)]
    pub total_tokens: i32,
    pub prompt_tokens_details: Option<Value>,
    pub prompt_cache_hit_tokens: Option<i32>,
    /// OpenRouter reports the provider's charge, including search fees, in USD.
    pub cost: Option<f64>,
}

impl TokenUsage {
    pub fn cached_tokens(&self) -> Option<i32> {
        self.prompt_tokens_details
            .as_ref()
            .and_then(|v| v.get("cached_tokens"))
            .and_then(Value::as_i64)
            .map(|v| v.clamp(0, self.prompt_tokens.max(0) as i64) as i32)
            .or_else(|| {
                self.prompt_cache_hit_tokens
                    .map(|v| v.clamp(0, self.prompt_tokens.max(0)))
            })
    }
}

impl From<openai_api_rs::v1::chat_completion::ChatCompletionResponse> for ChatResponse {
    fn from(r: openai_api_rs::v1::chat_completion::ChatCompletionResponse) -> Self {
        Self {
            id: r.id,
            object: r.object,
            created: r.created,
            model: r.model,
            choices: r.choices,
            system_fingerprint: r.system_fingerprint,
            headers: r.headers,
            usage: TokenUsage {
                prompt_tokens: r.usage.prompt_tokens,
                completion_tokens: r.usage.completion_tokens,
                total_tokens: r.usage.total_tokens,
                ..Default::default()
            },
        }
    }
}

/// Merge cumulative usage chunks; never add cumulative counts or discard cache/cost fields.
pub fn merge_stream_usage(target: &mut Value, chunk: &Value) {
    let Some(fields) = chunk.as_object() else {
        return;
    };
    if !target.is_object() {
        *target = serde_json::json!({});
    }
    for (key, value) in fields {
        if !value.is_null() {
            if value.is_object() {
                merge_stream_usage(&mut target[key], value);
            } else {
                target[key] = value.clone();
            }
        }
    }
}
