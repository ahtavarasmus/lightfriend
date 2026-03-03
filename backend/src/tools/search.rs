use openai_api_rs::v1::chat_completion;
use serde::{Deserialize, Serialize};

use crate::tools::registry::{ToolContext, ToolHandler, ToolResult};

// ─── ask_perplexity ──────────────────────────────────────────────────────────

pub struct PerplexityHandler;

#[derive(Deserialize, Serialize)]
struct PerplexityQuestion {
    query: String,
}

#[async_trait::async_trait]
impl ToolHandler for PerplexityHandler {
    fn name(&self) -> &'static str {
        "ask_perplexity"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::internet::get_ask_perplexity_tool()
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing ask_perplexity tool call");
        let c: PerplexityQuestion =
            serde_json::from_str(ctx.arguments).map_err(|e| e.to_string())?;
        let query = format!("User info: {}. Query: {}", ctx.user_given_info, c.query);
        let sys_prompt = format!(
            "You are assisting an AI text messaging service. The questions you receive are from text messaging conversations where users are seeking information or help. Please note: 1. Provide clear, conversational responses that can be easily read from a small screen 2. Avoid using any markdown, HTML, or other markup languages 3. Keep responses concise but informative 4. When listing multiple points, use simple numbering (1, 2, 3) 5. Focus on the most relevant information that addresses the user's immediate needs. This is what you should know about the user who this information is going to in their own words: {}",
            ctx.user_given_info
        );
        match crate::utils::tool_exec::ask_perplexity(ctx.state, &query, &sys_prompt).await {
            Ok(answer) => {
                tracing::debug!("Successfully received Perplexity answer");
                Ok(ToolResult::Answer(answer))
            }
            Err(e) => Err(e.to_string()),
        }
    }
}

// ─── search_firecrawl ────────────────────────────────────────────────────────

pub struct FirecrawlHandler;

#[derive(Deserialize, Serialize)]
struct FireCrawlQuestion {
    query: String,
}

#[async_trait::async_trait]
impl ToolHandler for FirecrawlHandler {
    fn name(&self) -> &'static str {
        "search_firecrawl"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::internet::get_firecrawl_search_tool()
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing search_firecrawl tool call");
        let c: FireCrawlQuestion =
            serde_json::from_str(ctx.arguments).map_err(|e| e.to_string())?;
        match crate::utils::tool_exec::handle_firecrawl_search(c.query, 5).await {
            Ok(answer) => {
                tracing::debug!("Successfully received fire crawl answer");
                Ok(ToolResult::Answer(answer))
            }
            Err(e) => Err(e.to_string()),
        }
    }
}

// ─── scan_qr_code ────────────────────────────────────────────────────────────

pub struct QrScanHandler;

#[async_trait::async_trait]
impl ToolHandler for QrScanHandler {
    fn name(&self) -> &'static str {
        "scan_qr_code"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::internet::get_scan_qr_code_tool()
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!(
            "Executing scan_qr_code tool call with url: {:#?}",
            ctx.image_url
        );
        let response = crate::tool_call_utils::internet::handle_qr_scan(ctx.image_url).await;
        Ok(ToolResult::Answer(response))
    }
}
