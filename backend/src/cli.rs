use clap::Parser;
use dotenvy::dotenv;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

const DEFAULT_SERVER_URL: &str = "https://lightfriend.ai";

#[derive(Parser, Debug)]
#[command(name = "backend")]
#[command(about = "Lightfriend backend CLI", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Parser, Debug)]
pub enum Commands {
    /// Configure ElevenLabs agent tools (server tools for create_item and get_items)
    ConfigureElevenlabsTools,
}

#[derive(Debug, Serialize, Deserialize)]
struct ElevenLabsTool {
    #[serde(rename = "toolId")]
    tool_id: String,
    name: String,
    #[serde(rename = "type")]
    tool_type: String,
    description: String,
    url: String,
    method: String,
    headers: Option<std::collections::HashMap<String, String>>,
    #[serde(rename = "parameters")]
    tool_parameters: Vec<ToolParameter>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolParameter {
    name: String,
    #[serde(rename = "type")]
    param_type: String,
    description: String,
    required: bool,
    #[serde(rename = "enum")]
    enum_values: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct AgentConfigResponse {
    #[serde(rename = "conversation_config")]
    conversation_config: ConversationConfig,
}

#[derive(Debug, Deserialize)]
struct ConversationConfig {
    #[serde(rename = "agent")]
    agent: serde_json::Value,
}

pub async fn run_cli() -> Result<bool, Box<dyn std::error::Error>> {
    dotenv().ok();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::ConfigureElevenlabsTools) => {
            configure_elevenlabs_tools().await?;
            Ok(true)
        }
        None => Ok(false),
    }
}

async fn configure_elevenlabs_tools() -> Result<(), Box<dyn std::error::Error>> {
    let agent_id = "lXHSLxMqPdUH87jVe9X4";
    let server_url = std::env::var("ELEVENLABS_TOOLS_SERVER_URL")
        .unwrap_or_else(|_| DEFAULT_SERVER_URL.to_string());
    let api_key =
        std::env::var("ELEVENLABS_API_KEY").expect("ELEVENLABS_API_KEY must be set in environment");

    let client = Client::new();

    println!("Fetching current agent configuration...");
    let response = client
        .get(format!(
            "https://api.elevenlabs.io/v1/convai/agents/{}",
            agent_id
        ))
        .header("xi-api-key", &api_key)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(format!("Failed to fetch agent: {}", error_text).into());
    }

    let agent_config: AgentConfigResponse = response.json().await?;

    let tools_array = agent_config
        .conversation_config
        .agent
        .get("prompt")
        .and_then(|p| p.get("tools"))
        .and_then(|t| t.as_array());

    let existing_tools: Vec<ElevenLabsTool> = match tools_array {
        Some(tools) => tools
            .iter()
            .filter_map(|t| serde_json::from_value(t.clone()).ok())
            .filter(|t: &ElevenLabsTool| {
                t.name != "create_item"
                    && t.name != "get_items"
                    && t.name != "CreateTask"
                    && t.name != "FetchTasks"
            })
            .collect(),
        None => vec![],
    };

    println!("Creating server tools for create_item and get_items...");

    let create_item_tool = ElevenLabsTool {
        tool_id: "create_item".to_string(),
        name: "create_item".to_string(),
        tool_type: "webhook".to_string(),
        description:
            "Create a new item (task, reminder, event, tracking, or recurring) for the user"
                .to_string(),
        url: format!("{}/api/call/items/create?user_id={{user_id}}", server_url),
        method: "POST".to_string(),
        headers: Some(
            [("Content-Type".to_string(), "application/json".to_string())]
                .into_iter()
                .collect(),
        ),
        tool_parameters: vec![
            ToolParameter {
                name: "item_type".to_string(),
                param_type: "string".to_string(),
                description: "Type of item: task, reminder, event, tracking, or recurring"
                    .to_string(),
                required: true,
                enum_values: Some(vec![
                    "task".to_string(),
                    "reminder".to_string(),
                    "event".to_string(),
                    "tracking".to_string(),
                    "recurring".to_string(),
                ]),
            },
            ToolParameter {
                name: "notify".to_string(),
                param_type: "string".to_string(),
                description: "Notification preference: silent, sms, call, or push".to_string(),
                required: true,
                enum_values: Some(vec![
                    "silent".to_string(),
                    "sms".to_string(),
                    "call".to_string(),
                    "push".to_string(),
                ]),
            },
            ToolParameter {
                name: "description".to_string(),
                param_type: "string".to_string(),
                description: "Description or title of the item".to_string(),
                required: true,
                enum_values: None,
            },
            ToolParameter {
                name: "due_at".to_string(),
                param_type: "string".to_string(),
                description: "Due date/time in ISO 8601 format (optional)".to_string(),
                required: false,
                enum_values: None,
            },
            ToolParameter {
                name: "repeat".to_string(),
                param_type: "string".to_string(),
                description:
                    "Repeat interval (e.g., daily, weekly, monthly) for recurring items (optional)"
                        .to_string(),
                required: false,
                enum_values: None,
            },
            ToolParameter {
                name: "fetch".to_string(),
                param_type: "string".to_string(),
                description: "Fetch interval for tracking items (optional)".to_string(),
                required: false,
                enum_values: None,
            },
            ToolParameter {
                name: "platform".to_string(),
                param_type: "string".to_string(),
                description:
                    "Platform to track (e.g., whatsapp, telegram) for tracking items (optional)"
                        .to_string(),
                required: false,
                enum_values: None,
            },
            ToolParameter {
                name: "sender".to_string(),
                param_type: "string".to_string(),
                description:
                    "Sender to track (e.g., a specific contact) for tracking items (optional)"
                        .to_string(),
                required: false,
                enum_values: None,
            },
            ToolParameter {
                name: "topic".to_string(),
                param_type: "string".to_string(),
                description: "Topic to track for tracking items (optional)".to_string(),
                required: false,
                enum_values: None,
            },
        ],
    };

    let get_items_tool = ElevenLabsTool {
        tool_id: "get_items".to_string(),
        name: "get_items".to_string(),
        tool_type: "webhook".to_string(),
        description: "Get all items for the user".to_string(),
        url: format!("{}/api/call/items?user_id={{user_id}}", server_url),
        method: "GET".to_string(),
        headers: None,
        tool_parameters: vec![],
    };

    let mut all_tools = existing_tools;
    all_tools.push(create_item_tool);
    all_tools.push(get_items_tool);

    println!("Updating agent with new tools configuration...");

    let update_payload = json!({
        "conversation_config": {
            "agent": {
                "prompt": {
                    "tools": all_tools
                }
            }
        }
    });

    let update_response = client
        .patch(format!(
            "https://api.elevenlabs.io/v1/convai/agents/{}",
            agent_id
        ))
        .header("xi-api-key", &api_key)
        .header("Content-Type", "application/json")
        .json(&update_payload)
        .send()
        .await?;

    if !update_response.status().is_success() {
        let error_text = update_response.text().await?;
        return Err(format!("Failed to update agent: {}", error_text).into());
    }

    println!("Successfully configured ElevenLabs agent tools!");
    println!("  - create_item: POST {}/api/call/items/create", server_url);
    println!("  - get_items: GET {}/api/call/items", server_url);

    Ok(())
}
