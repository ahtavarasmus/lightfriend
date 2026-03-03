use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::{handlers::auth_middleware::AuthUser, AppState};

// Item edit with AI
#[derive(Deserialize)]
pub struct ItemEditRequest {
    pub instruction: String,
}

#[derive(Deserialize)]
pub struct ItemEditQuery {
    pub instruction: String,
}

#[derive(Serialize)]
pub struct ItemEditResponse {
    pub message: String,
    pub success: bool,
}

/// AI response structure for item editing
#[derive(Deserialize)]
struct AiItemEditResult {
    new_trigger_time: Option<String>,
    new_summary: Option<String>,
    explanation: String,
}

pub async fn edit_item_with_ai(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(item_id): Path<i32>,
    Json(request): Json<ItemEditRequest>,
) -> Result<Json<ItemEditResponse>, (StatusCode, Json<serde_json::Value>)> {
    use openai_api_rs::v1::chat_completion::{
        ChatCompletionMessage, ChatCompletionRequest, Content, MessageRole,
    };

    tracing::info!(
        "edit_item_with_ai called: item_id={}, user_id={}, instruction={}",
        item_id,
        auth_user.user_id,
        request.instruction
    );

    // Get the item and verify ownership
    let item = state
        .item_repository
        .get_item(item_id, auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to get item: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Item not found"})),
            )
        })?;

    // Build context for LLM client and timezone
    let ctx = crate::context::ContextBuilder::for_user(&state, auth_user.user_id)
        .with_user_context()
        .build()
        .await
        .map_err(|e| {
            tracing::error!("Failed to build context: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "AI service unavailable"})),
            )
        })?;

    let user_tz_str = ctx
        .timezone
        .as_ref()
        .map(|tz| tz.tz_str.clone())
        .unwrap_or_else(|| "UTC".to_string());
    let formatted_now = ctx
        .timezone
        .as_ref()
        .map(|tz| tz.formatted_now.clone())
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string());

    // Format current scheduled time in user's timezone
    let tz: chrono_tz::Tz = user_tz_str.parse().unwrap_or(chrono_tz::UTC);
    let current_time = item
        .due_at
        .and_then(|nca| chrono::DateTime::from_timestamp(nca as i64, 0))
        .map(|dt| dt.with_timezone(&tz).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "Not scheduled".to_string());

    let tags = crate::proactive::utils::parse_summary_tags(&item.summary);
    let item_type = if tags.item_type.as_deref() == Some("tracking") {
        "tracking (watches incoming data)"
    } else {
        "scheduled item"
    };

    let system_prompt = format!(
        r#"You are editing an item tracked by an AI assistant.

CURRENT ITEM:
- Type: {}
- Summary: {}
- Scheduled time: {} (timezone: {})

USER'S EDIT REQUEST: "{}"

Return ONLY valid JSON (no markdown, no code blocks):
{{
  "new_trigger_time": "2025-06-15T14:30:00" or null,
  "new_summary": "updated summary text" or null,
  "explanation": "Brief explanation"
}}

RULES:
- new_trigger_time: Only set if user wants to change the scheduled time. Use exact format YYYY-MM-DDTHH:MM:SS
- new_summary: Only set if user wants to change what the item does/tracks
- explanation: Always provide a brief explanation of the change
- Return null for unchanged fields

TIME RULES:
- Calculate actual datetime for "tomorrow", "9am", "in 2 hours", etc.
- Timezone: {}
- Current time: {}
- Return null if no time change"#,
        item_type,
        item.summary,
        current_time,
        user_tz_str,
        request.instruction,
        user_tz_str,
        formatted_now
    );

    let messages = vec![ChatCompletionMessage {
        role: MessageRole::user,
        content: Content::Text(system_prompt),
        name: None,
        tool_calls: None,
        tool_call_id: None,
    }];

    let ai_request = ChatCompletionRequest::new(ctx.model.clone(), messages).temperature(0.1);

    let response = ctx.client.chat_completion(ai_request).await.map_err(|e| {
        tracing::error!("AI request failed: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("AI request failed: {}", e)})),
        )
    })?;

    let ai_response = response
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "No response from AI"})),
            )
        })?;

    tracing::debug!("AI response for item edit: {}", ai_response);

    // Clean up AI response - remove markdown code blocks if present
    let cleaned_response = ai_response
        .trim()
        .strip_prefix("```json")
        .or_else(|| ai_response.trim().strip_prefix("```"))
        .unwrap_or(&ai_response)
        .trim()
        .strip_suffix("```")
        .unwrap_or(&ai_response)
        .trim();

    let edit_result: AiItemEditResult = serde_json::from_str(cleaned_response).map_err(|e| {
        tracing::error!(
            "Failed to parse AI response: {} - Response was: {}",
            e,
            cleaned_response
        );
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to understand edit instruction. Please try rephrasing."})),
        )
    })?;

    let mut changes_made = false;

    // Update trigger time if specified
    if let Some(new_time_str) = &edit_result.new_trigger_time {
        let tz: chrono_tz::Tz = user_tz_str.parse().unwrap_or(chrono_tz::UTC);

        let parsed = chrono::NaiveDateTime::parse_from_str(new_time_str, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(new_time_str, "%Y-%m-%dT%H:%M"));

        match parsed {
            Ok(naive_dt) => {
                use chrono::TimeZone;
                if let Some(local_dt) = tz.from_local_datetime(&naive_dt).single() {
                    let new_ts = local_dt.timestamp() as i32;
                    state
                        .item_repository
                        .update_due_at(item_id, Some(new_ts))
                        .map_err(|e| {
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!({"error": format!("Failed to update time: {}", e)})),
                            )
                        })?;
                    changes_made = true;
                }
            }
            Err(e) => {
                tracing::error!("Failed to parse AI time '{}': {}", new_time_str, e);
            }
        }
    }

    // Update summary if specified
    if let Some(new_summary) = &edit_result.new_summary {
        state
            .item_repository
            .update_summary(item_id, new_summary)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to update summary: {}", e)})),
                )
            })?;
        changes_made = true;
    }

    if changes_made {
        Ok(Json(ItemEditResponse {
            message: edit_result.explanation,
            success: true,
        }))
    } else {
        Ok(Json(ItemEditResponse {
            message: "No changes were made. Please clarify what you'd like to change.".to_string(),
            success: false,
        }))
    }
}

/// SSE streaming version of edit_item_with_ai - sends status updates so the UI doesn't appear stuck.
pub async fn edit_item_with_ai_stream(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(item_id): Path<i32>,
    axum::extract::Query(request): axum::extract::Query<ItemEditQuery>,
) -> axum::response::sse::Sse<
    impl futures::stream::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>,
> {
    use axum::response::sse::Event;

    let stream = async_stream::stream! {
        yield Ok(Event::default().data(
            json!({"step": "thinking", "message": "Thinking..."}).to_string(),
        ));

        // Get the item and verify ownership
        let item = match state.item_repository.get_item(item_id, auth_user.user_id) {
            Ok(Some(item)) => item,
            Ok(None) => {
                yield Ok(Event::default().data(
                    json!({"step": "error", "message": "Item not found"}).to_string(),
                ));
                return;
            }
            Err(e) => {
                yield Ok(Event::default().data(
                    json!({"step": "error", "message": format!("Database error: {}", e)}).to_string(),
                ));
                return;
            }
        };

        // Build context
        let ctx = match crate::context::ContextBuilder::for_user(&state, auth_user.user_id)
            .with_user_context()
            .build()
            .await
        {
            Ok(ctx) => ctx,
            Err(e) => {
                tracing::error!("Failed to build context: {}", e);
                yield Ok(Event::default().data(
                    json!({"step": "error", "message": "AI service unavailable"}).to_string(),
                ));
                return;
            }
        };

        let user_tz_str = ctx.timezone.as_ref()
            .map(|tz| tz.tz_str.clone())
            .unwrap_or_else(|| "UTC".to_string());
        let formatted_now = ctx.timezone.as_ref()
            .map(|tz| tz.formatted_now.clone())
            .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string());

        // Format current scheduled time in user's timezone
        let tz: chrono_tz::Tz = user_tz_str.parse().unwrap_or(chrono_tz::UTC);
        let current_time = item.due_at
            .and_then(|nca| chrono::DateTime::from_timestamp(nca as i64, 0))
            .map(|dt| dt.with_timezone(&tz).format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "Not scheduled".to_string());

        let tags = crate::proactive::utils::parse_summary_tags(&item.summary);
        let item_type = if tags.item_type.as_deref() == Some("tracking") {
            "tracking (watches incoming data)"
        } else {
            "scheduled item"
        };

        let system_prompt = format!(
            r#"You are editing an item tracked by an AI assistant.

CURRENT ITEM:
- Type: {}
- Summary: {}
- Scheduled time: {} (timezone: {})

USER'S EDIT REQUEST: "{}"

Return ONLY valid JSON (no markdown, no code blocks):
{{
  "new_trigger_time": "2025-06-15T14:30:00" or null,
  "new_summary": "updated summary text" or null,
  "explanation": "Brief explanation"
}}

RULES:
- new_trigger_time: Only set if user wants to change the scheduled time. Use exact format YYYY-MM-DDTHH:MM:SS
- new_summary: Only set if user wants to change what the item does/tracks
- explanation: Always provide a brief explanation of the change
- Return null for unchanged fields

TIME RULES:
- Calculate actual datetime for "tomorrow", "9am", "in 2 hours", etc.
- Timezone: {}
- Current time: {}
- Return null if no time change"#,
            item_type, item.summary, current_time, user_tz_str,
            request.instruction, user_tz_str, formatted_now
        );

        use openai_api_rs::v1::chat_completion::{
            ChatCompletionMessage, ChatCompletionRequest, Content, MessageRole,
        };

        let messages = vec![ChatCompletionMessage {
            role: MessageRole::user,
            content: Content::Text(system_prompt),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }];

        let ai_request = ChatCompletionRequest::new(ctx.model.clone(), messages).temperature(0.1);

        // Stream reasoning tokens while the AI processes the edit
        let (reasoning_tx, mut reasoning_rx) = tokio::sync::mpsc::channel::<String>(32);
        let provider = ctx.provider;
        let state_for_ai = state.clone();
        let mut ai_handle = tokio::spawn(async move {
            state_for_ai.ai_config.chat_completion_streaming(provider, &ai_request, Some(reasoning_tx)).await
        });

        #[allow(unused_assignments)]
        let mut ai_result = None;
        loop {
            tokio::select! {
                snippet = reasoning_rx.recv() => {
                    match snippet {
                        Some(text) => {
                            yield Ok(Event::default().data(
                                json!({"step": "reasoning", "message": text}).to_string(),
                            ));
                        }
                        None => {
                            // Channel closed - AI call dropped the sender
                            ai_result = Some(ai_handle.await);
                            break;
                        }
                    }
                }
                result = &mut ai_handle => {
                    tokio::task::yield_now().await;
                    // Drain remaining reasoning snippets
                    while let Ok(text) = reasoning_rx.try_recv() {
                        yield Ok(Event::default().data(
                            json!({"step": "reasoning", "message": text}).to_string(),
                        ));
                    }
                    ai_result = Some(result);
                    break;
                }
            }
        }

        let response = match ai_result {
            Some(Ok(Ok(r))) => r,
            Some(Ok(Err(e))) => {
                tracing::error!("AI request failed: {}", e);
                yield Ok(Event::default().data(
                    json!({"step": "error", "message": format!("AI request failed: {}", e)}).to_string(),
                ));
                return;
            }
            Some(Err(e)) => {
                tracing::error!("AI task panicked: {}", e);
                yield Ok(Event::default().data(
                    json!({"step": "error", "message": "AI request failed unexpectedly"}).to_string(),
                ));
                return;
            }
            None => {
                yield Ok(Event::default().data(
                    json!({"step": "error", "message": "AI request failed unexpectedly"}).to_string(),
                ));
                return;
            }
        };

        let ai_response = match response.choices.first().and_then(|c| c.message.content.clone()) {
            Some(text) => text,
            None => {
                yield Ok(Event::default().data(
                    json!({"step": "error", "message": "No response from AI"}).to_string(),
                ));
                return;
            }
        };

        tracing::debug!("AI response for item edit (stream): {}", ai_response);

        let cleaned_response = ai_response
            .trim()
            .strip_prefix("```json")
            .or_else(|| ai_response.trim().strip_prefix("```"))
            .unwrap_or(&ai_response)
            .trim()
            .strip_suffix("```")
            .unwrap_or(&ai_response)
            .trim();

        let edit_result: AiItemEditResult = match serde_json::from_str(cleaned_response) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Failed to parse AI response: {} - Response was: {}", e, cleaned_response);
                yield Ok(Event::default().data(
                    json!({"step": "error", "message": "Failed to understand edit instruction. Please try rephrasing."}).to_string(),
                ));
                return;
            }
        };

        let mut changes_made = false;

        if let Some(new_time_str) = &edit_result.new_trigger_time {
            let tz: chrono_tz::Tz = user_tz_str.parse().unwrap_or(chrono_tz::UTC);
            let parsed = chrono::NaiveDateTime::parse_from_str(new_time_str, "%Y-%m-%dT%H:%M:%S")
                .or_else(|_| chrono::NaiveDateTime::parse_from_str(new_time_str, "%Y-%m-%dT%H:%M"));

            match parsed {
                Ok(naive_dt) => {
                    use chrono::TimeZone;
                    if let Some(local_dt) = tz.from_local_datetime(&naive_dt).single() {
                        let new_ts = local_dt.timestamp() as i32;
                        if let Err(e) = state.item_repository.update_due_at(item_id, Some(new_ts)) {
                            yield Ok(Event::default().data(
                                json!({"step": "error", "message": format!("Failed to update time: {}", e)}).to_string(),
                            ));
                            return;
                        }
                        changes_made = true;
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to parse AI time '{}': {}", new_time_str, e);
                }
            }
        }

        if let Some(new_summary) = &edit_result.new_summary {
            if let Err(e) = state.item_repository.update_summary(item_id, new_summary) {
                yield Ok(Event::default().data(
                    json!({"step": "error", "message": format!("Failed to update summary: {}", e)}).to_string(),
                ));
                return;
            }
            changes_made = true;
        }

        let message = if changes_made {
            edit_result.explanation
        } else {
            "No changes were made. Please clarify what you'd like to change.".to_string()
        };

        yield Ok(Event::default().data(
            json!({"step": "complete", "message": message}).to_string(),
        ));
    };

    axum::response::sse::Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new().interval(std::time::Duration::from_secs(15)),
    )
}
