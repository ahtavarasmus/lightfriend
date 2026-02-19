use openai_api_rs::v1::chat_completion;
use serde::{Deserialize, Serialize};

use crate::tools::registry::{ToolContext, ToolHandler, ToolResult};

pub struct WeatherHandler;

#[derive(Deserialize, Serialize)]
struct WeatherQuestion {
    location: String,
    units: String,
    forecast_type: Option<String>,
}

#[async_trait::async_trait]
impl ToolHandler for WeatherHandler {
    fn name(&self) -> &'static str {
        "get_weather"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::internet::get_weather_tool()
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing get_weather tool call");
        let c: WeatherQuestion = serde_json::from_str(ctx.arguments).map_err(|e| e.to_string())?;
        let forecast_type = c.forecast_type.unwrap_or_else(|| "current".to_string());
        match crate::utils::tool_exec::get_weather(
            ctx.state,
            &c.location,
            &c.units,
            &forecast_type,
            ctx.user_id,
        )
        .await
        {
            Ok(answer) => {
                tracing::debug!("Successfully received weather answer");
                Ok(ToolResult::Answer(answer))
            }
            Err(e) => Err(e.to_string()),
        }
    }
}
