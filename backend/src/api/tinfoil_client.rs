use reqwest::multipart;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::AiProvider;

/// A message in the voice pipeline's conversation history.
/// Supports text, assistant-with-tool-calls, and tool-result roles.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    /// For assistant messages that include tool calls
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallInfo>>,
    /// For tool result messages
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn user(content: &str) -> Self {
        Self {
            role: "user".to_string(),
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: None,
        }
    }
    pub fn assistant(content: &str) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: None,
        }
    }
    pub fn assistant_with_tool_calls(content: &str, tool_calls: Vec<ToolCallInfo>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.to_string(),
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }
    pub fn tool_result(tool_call_id: &str, content: &str) -> Self {
        Self {
            role: "tool".to_string(),
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.to_string()),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolCallInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ToolCallFunction,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

/// The result of a chat completion - either text or tool calls.
pub enum CompletionResult {
    Text(String),
    ToolCalls {
        content: String,
        tool_calls: Vec<ToolCallInfo>,
    },
}

/// HTTP client wrapping Tinfoil's STT, LLM, and TTS endpoints.
pub struct TinfoilVoiceClient {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

/// Sanitize content for Tinfoil API (same logic as AiConfig::sanitize_content).
fn sanitize_for_tinfoil(s: &str) -> String {
    s.replace('@', "(at)")
}

/// Convert a ChatMessage to its JSON representation for the API.
fn message_to_json(msg: &ChatMessage) -> serde_json::Value {
    let mut m = serde_json::json!({
        "role": msg.role,
        "content": sanitize_for_tinfoil(&msg.content),
    });
    if let Some(ref tool_calls) = msg.tool_calls {
        m["tool_calls"] = serde_json::to_value(tool_calls).unwrap_or_default();
    }
    if let Some(ref id) = msg.tool_call_id {
        m["tool_call_id"] = serde_json::json!(id);
    }
    m
}

impl TinfoilVoiceClient {
    pub fn new(ai_config: &crate::AiConfig) -> Self {
        let base_url = ai_config.endpoint(AiProvider::Tinfoil).to_string();
        let api_key = ai_config.api_key(AiProvider::Tinfoil).to_string();

        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(4)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            client,
            base_url,
            api_key,
        }
    }

    /// Transcribe audio via Whisper.
    pub async fn transcribe(&self, audio_wav: &[u8]) -> Result<String, String> {
        let url = format!("{}/audio/transcriptions", self.base_url);

        let part = multipart::Part::bytes(audio_wav.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| format!("multipart error: {}", e))?;

        let form = multipart::Form::new()
            .text("model", "whisper-large-v3-turbo")
            .part("file", part);

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| format!("STT request failed: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("STT error {}: {}", status, body));
        }

        let parsed: TranscriptionResponse = resp
            .json()
            .await
            .map_err(|e| format!("STT parse error: {}", e))?;

        Ok(parsed.text)
    }

    /// Chat completion with optional tool definitions.
    /// Returns either text content or tool calls.
    pub async fn chat_completion_with_tools(
        &self,
        messages: &[ChatMessage],
        system_prompt: &str,
        tools: Option<&[serde_json::Value]>,
        model: &str,
    ) -> Result<CompletionResult, String> {
        let url = format!("{}/chat/completions", self.base_url);

        let mut all_messages = vec![serde_json::json!({
            "role": "system",
            "content": sanitize_for_tinfoil(system_prompt),
        })];
        for msg in messages {
            all_messages.push(message_to_json(msg));
        }

        let mut body = serde_json::json!({
            "model": model,
            "messages": all_messages,
            "temperature": 0.3,
            "max_tokens": 500,
        });

        if let Some(tool_defs) = tools {
            if !tool_defs.is_empty() {
                body["tools"] = serde_json::Value::Array(tool_defs.to_vec());
            }
        }

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .timeout(Duration::from_secs(45))
            .send()
            .await
            .map_err(|e| format!("LLM request failed: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("LLM error {}: {}", status, body));
        }

        let parsed: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("LLM parse error: {}", e))?;

        let choice = parsed["choices"]
            .as_array()
            .and_then(|c| c.first())
            .ok_or("LLM returned no choices")?;

        let content = choice["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // Check for tool calls
        if let Some(tc_array) = choice["message"]["tool_calls"].as_array() {
            if !tc_array.is_empty() {
                let tool_calls: Vec<ToolCallInfo> = tc_array
                    .iter()
                    .filter_map(|tc| {
                        Some(ToolCallInfo {
                            id: tc["id"].as_str()?.to_string(),
                            call_type: tc["type"].as_str().unwrap_or("function").to_string(),
                            function: ToolCallFunction {
                                name: tc["function"]["name"].as_str()?.to_string(),
                                arguments: tc["function"]["arguments"]
                                    .as_str()
                                    .unwrap_or("{}")
                                    .to_string(),
                            },
                        })
                    })
                    .collect();

                if !tool_calls.is_empty() {
                    return Ok(CompletionResult::ToolCalls {
                        content,
                        tool_calls,
                    });
                }
            }
        }

        if content.is_empty() {
            return Err("LLM returned no content".to_string());
        }

        Ok(CompletionResult::Text(content))
    }

    /// Simple chat completion without tools (convenience wrapper).
    pub async fn chat_completion(
        &self,
        messages: &[ChatMessage],
        system_prompt: &str,
        model: &str,
    ) -> Result<String, String> {
        match self
            .chat_completion_with_tools(messages, system_prompt, None, model)
            .await?
        {
            CompletionResult::Text(t) => Ok(t),
            CompletionResult::ToolCalls { content, .. } => {
                // Shouldn't happen without tools, but return content if available
                if content.is_empty() {
                    Err("LLM returned tool calls without tools defined".to_string())
                } else {
                    Ok(content)
                }
            }
        }
    }

    /// Text-to-speech. Returns raw WAV bytes.
    pub async fn text_to_speech(&self, text: &str, voice: &str) -> Result<Vec<u8>, String> {
        let url = format!("{}/audio/speech", self.base_url);

        let body = serde_json::json!({
            "model": "qwen3-tts",
            "input": text,
            "voice": voice,
            "response_format": "wav",
        });

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| format!("TTS request failed: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("TTS error {}: {}", status, body));
        }

        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| format!("TTS read error: {}", e))
    }
}
