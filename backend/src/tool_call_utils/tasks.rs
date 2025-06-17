use crate::AppState;
use std::sync::Arc;
use serde::Deserialize;
use axum::Json;
use chrono::{DateTime, Utc};


pub fn get_fetch_tasks_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut tasks_properties = HashMap::new();
    tasks_properties.insert(
        "param".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Can be anything, will fetch all tasks regardless".to_string()),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("fetch_tasks"),
            description: Some(String::from("Fetches the user's Google Tasks. Use this when user asks about their tasks, or ideas.")),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(tasks_properties),
                required: None,
            },
        },
    }
}


pub fn get_create_tasks_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;


    let mut create_task_properties = HashMap::new();
    create_task_properties.insert(
        "title".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The title of the task".to_string()),
            ..Default::default()
        }),
    );
    create_task_properties.insert(
        "description".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Optional description of the task".to_string()),
            ..Default::default()
        }),
    );
    create_task_properties.insert(
        "due_time".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Optional due time in RFC3339 format in UTC (e.g., '2024-03-23T14:30:00Z')".to_string()),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("create_task"),
            description: Some(String::from("Creates a new Google Task. Use this when user wants to add a task, reminder, or idea.")),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(create_task_properties),
                required: Some(vec![String::from("title")]),
            },
        },
    }
}


#[derive(Deserialize)]
pub struct CreateTaskArgs {
    pub title: String,
    pub description: Option<String>,
    pub due_time: Option<String>,
}


pub async fn handle_fetch_tasks(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
) -> String {
    match crate::handlers::google_tasks::get_tasks(state, user_id).await {
        Ok(Json(response)) => {
            if let Some(tasks) = response.get("tasks") {
                if let Some(tasks_array) = tasks.as_array() {
                    if tasks_array.is_empty() {
                        "You don't have any tasks in your list.".to_string()
                    } else {
                        let mut response = String::new();
                        for (i, task) in tasks_array.iter().enumerate() {
                            let title = task.get("title").and_then(|t| t.as_str()).unwrap_or("Untitled");
                            let status = task.get("status").and_then(|s| s.as_str()).unwrap_or("unknown");
                            let due = task.get("due").and_then(|d| d.as_str()).unwrap_or("");
                            let notes = task.get("notes").and_then(|n| n.as_str()).unwrap_or("");
                            
                            let status_emoji = if status == "completed" { "✅" } else { "📝" };
                            let due_text = if !due.is_empty() {
                                format!(" (due: {})", due)
                            } else {
                                String::new()
                            };
                            
                            if i == 0 {
                                response.push_str(&format!("{}. {} {}{}", i + 1, status_emoji, title, due_text));
                            } else {
                                response.push_str(&format!("\n{}. {} {}{}", i + 1, status_emoji, title, due_text));
                            }
                            
                            if !notes.is_empty() {
                                response.push_str(&format!("\n   Note: {}", notes));
                            }
                        }
                        response
                    }
                } else {
                    "Failed to parse tasks list.".to_string()
                }
            } else {
                "No tasks found.".to_string()
            }
        }
        Err((status, Json(error))) => {
            let error_message = match status {
                axum::http::StatusCode::UNAUTHORIZED => "You need to connect your Google Tasks first. Visit the website to set it up.",
                _ => "Failed to fetch tasks. Please try again later.",
            };
            eprintln!("Failed to fetch tasks: {:?}", error);
            error_message.to_string()
        }
    }
}

pub async fn handle_create_task(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
) -> String {
    let args: CreateTaskArgs = match serde_json::from_str(args) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("Failed to parse create task arguments: {}", e);
            return "Failed to create task due to invalid arguments.".to_string();
        }
    };

    // Convert due_time string to DateTime<Utc> if provided
    let due_time = if let Some(dt_str) = args.due_time {
        match chrono::DateTime::parse_from_rfc3339(&dt_str) {
            Ok(dt) => Some(dt.with_timezone(&chrono::Utc)),
            Err(e) => {
                eprintln!("Failed to parse due time: {}", e);
                None
            }
        }
    } else {
        None
    };

    let task_request = crate::handlers::google_tasks::CreateTaskRequest {
        title: args.title,
        description: args.description,
        due_time,
    };

    match crate::handlers::google_tasks::create_task(state, user_id, &task_request).await {
        Ok(Json(response)) => "Task created successfully.".to_string(),
        Err((status, Json(error))) => {
            let error_message = match status {
                axum::http::StatusCode::UNAUTHORIZED => "You need to connect your Google Tasks first. Visit the website to set it up.",
                _ => "Failed to create task. Please try again later.",
            };
            eprintln!("Failed to create task: {:?}", error);
            error_message.to_string()
        }
    }
}

