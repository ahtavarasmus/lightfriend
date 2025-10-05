use std::sync::Arc;
use crate::handlers::auth_middleware::AuthUser;
use axum::{
    extract::State,
    response::Json,
    http::StatusCode,
};
use oauth2::TokenResponse;
use serde::{Deserialize, Serialize};
use serde_json::json;
use chrono::{DateTime, Utc};
use reqwest::header::{AUTHORIZATION, ACCEPT};

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    pub description: Option<String>,
    pub due_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskList {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub due: Option<String>,
    #[serde(default, rename = "due_time")]
    pub due_time: Option<String>,
    pub status: String,
}

#[derive(Debug, Deserialize)]
struct TaskListResponse {
    items: Option<Vec<TaskList>>,
}

#[derive(Debug, Deserialize)]
struct TasksResponse {
    items: Option<Vec<Task>>,
}

const LIGHTFRIEND_LIST_NAME: &str = "lightfriend";

#[derive(Debug)]
pub enum TaskError {
    NoConnection,
    TokenError(String),
    ApiError(String),
    ParseError(String),
}

impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskError::NoConnection => write!(f, "No active Google Tasks connection"),
            TaskError::TokenError(msg) => write!(f, "Token error: {}", msg),
            TaskError::ApiError(msg) => write!(f, "API error: {}", msg),
            TaskError::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

pub async fn google_tasks_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Checking Google Tasks connection status");

    // Check if user has active Google Calendar connection
    match state.user_repository.has_active_google_tasks() {
        Ok(has_connection) => {
            tracing::info!("Successfully checked tasks connection status for user {}: {}", auth_user.user_id, has_connection);
            Ok(Json(json!({
                "connected": has_connection,
                "user_id": auth_user.user_id,
            })))
        },
        Err(e) => {
            tracing::error!("Failed to check tasks connection status: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to check tasks connection status",
                    "details": e.to_string()
                 }))
            ))
        }
    }
}


async fn ensure_lightfriend_list(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<String, TaskError> {
    // First, try to find existing lightfriend list
    let response = client
        .get("https://tasks.googleapis.com/tasks/v1/users/@me/lists")
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .header(ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| TaskError::ApiError(e.to_string()))?;

    if !response.status().is_success() {
        return Err(TaskError::ApiError(format!(
            "Failed to fetch task lists: {}",
            response.status()
        )));
    }

    let task_lists: TaskListResponse = response
        .json()
        .await
        .map_err(|e| TaskError::ParseError(e.to_string()))?;

    // Check if lightfriend list already exists
    if let Some(lists) = task_lists.items {
        if let Some(list) = lists.iter().find(|l| l.title == LIGHTFRIEND_LIST_NAME) {
            return Ok(list.id.clone());
        }
    }

    // Create new lightfriend list
    let response = client
        .post("https://tasks.googleapis.com/tasks/v1/users/@me/lists")
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .header(ACCEPT, "application/json")
        .json(&json!({
            "title": LIGHTFRIEND_LIST_NAME
        }))
        .send()
        .await
        .map_err(|e| TaskError::ApiError(e.to_string()))?;

    if !response.status().is_success() {
        return Err(TaskError::ApiError(format!(
            "Failed to create task list: {}",
            response.status()
        )));
    }

    let new_list: TaskList = response
        .json()
        .await
        .map_err(|e| TaskError::ParseError(e.to_string()))?;

    Ok(new_list.id)
}

async fn create_task_with_token(
    client: &reqwest::Client,
    access_token: &str,
    task_request: &CreateTaskRequest,
) -> Result<Task, TaskError> {
    // Ensure lightfriend list exists and get its ID
    let list_id = ensure_lightfriend_list(client, access_token).await?;

    // Create task
    let mut task_data = json!({
        "title": task_request.title,
        "status": "needsAction"
    });

    if let Some(desc) = &task_request.description {
        task_data["notes"] = json!(desc);
    }

    if let Some(due) = task_request.due_time {
        // Set both due (date) and due_time (specific time) parameters
        task_data["due"] = json!(due.to_rfc3339());
        task_data["due_time"] = json!(due.to_rfc3339());
    }

    let response = client
        .post(format!(
            "https://tasks.googleapis.com/tasks/v1/lists/{}/tasks",
            list_id
        ))
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .header(ACCEPT, "application/json")
        .json(&task_data)
        .send()
        .await
        .map_err(|e| TaskError::ApiError(e.to_string()))?;

    if !response.status().is_success() {
        return Err(TaskError::ApiError(format!(
            "Failed to create task: {}",
            response.status()
        )));
    }

    let task: Task = response
        .json()
        .await
        .map_err(|e| TaskError::ParseError(e.to_string()))?;

    Ok(task)
}

pub async fn create_task(
    state: &AppState,
    user_id: i32,
    task_request: &CreateTaskRequest,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Get Google Tasks tokens
    let (access_token, refresh_token) = match state.user_repository.get_google_tasks_tokens() {
        Ok(Some((access, refresh))) => (access, refresh),
        Ok(None) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "No active Google Tasks connection"}))
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get tasks tokens: {}", e)}))
            ));
        }
    };

    let client = reqwest::Client::new();

    // First attempt with current token
    let result = create_task_with_token(&client, &access_token, &task_request).await;

    let task = match result {
        Ok(task) => task,
        Err(TaskError::ApiError(e)) if e.contains("401") => {
            tracing::info!("Access token expired, attempting to refresh");
            
            let http_client = reqwest::ClientBuilder::new()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("Client should build");

            // Refresh the token
            let token_result = state
                .google_tasks_oauth_client
                .exchange_refresh_token(&oauth2::RefreshToken::new(refresh_token.clone()))
                .request_async(&http_client)
                .await
                .map_err(|e| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to refresh token: {}", e)}))
                ))?;

            let new_access_token = token_result.access_token().secret();
            let expires_in = token_result.expires_in()
                .unwrap_or_default()
                .as_secs() as i32;

            // Update the access token in the database
            state.user_repository.update_google_tasks_access_token(
                new_access_token.clone().as_str(),
                expires_in,
            ).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to update token: {}", e)}))
            ))?;

            // Retry with new token
            create_task_with_token(&client, &new_access_token, &task_request)
                .await
                .map_err(|e| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to create task after token refresh: {}", e)}))
                ))?
        },
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to create task: {}", e)}))
            ));
        }
    };


    Ok(Json(json!({
        "message": "Task created successfully",
        "task": task
    })))
}


pub async fn handle_tasks_creation_route(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(task_request): Json<CreateTaskRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    // Call the existing handler function
    let res = create_task(&state, auth_user.user_id, &task_request).await;
    println!("res: {:#?}", res);
    res
}

pub async fn handle_tasks_fetching_route(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    // Call the existing handler function
    let res = get_tasks(&state, auth_user.user_id).await;
    println!("res: {:#?}", res);
    res
}

async fn fetch_tasks_with_token(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<Vec<Task>, TaskError> {
    // Ensure lightfriend list exists and get its ID
    let list_id = ensure_lightfriend_list(client, access_token).await?;

    // Fetch tasks
    let response = client
        .get(format!(
            "https://tasks.googleapis.com/tasks/v1/lists/{}/tasks",
            list_id
        ))
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .header(ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| TaskError::ApiError(e.to_string()))?;

    if !response.status().is_success() {
        return Err(TaskError::ApiError(format!(
            "Failed to fetch tasks: {}",
            response.status()
        )));
    }

    let tasks: TasksResponse = response
        .json()
        .await
        .map_err(|e| TaskError::ParseError(e.to_string()))?;

    Ok(tasks.items.unwrap_or_default())
}

pub async fn get_tasks(
    state: &AppState,
    user_id: i32,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Get Google Tasks tokens
    let (access_token, refresh_token) = match state.user_repository.get_google_tasks_tokens() {
        Ok(Some((access, refresh))) => (access, refresh),
        Ok(None) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "No active Google Tasks connection"}))
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get tasks tokens: {}", e)}))
            ));
        }
    };

    let client = reqwest::Client::new();

    // First attempt with current token
    let result = fetch_tasks_with_token(&client, &access_token).await;

    let tasks = match result {
        Ok(tasks) => tasks,
        Err(TaskError::ApiError(e)) if e.contains("401") => {
            tracing::info!("Access token expired, attempting to refresh");
            
            let http_client = reqwest::ClientBuilder::new()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("Client should build");

            // Refresh the token
            let token_result = state
                .google_tasks_oauth_client
                .exchange_refresh_token(&oauth2::RefreshToken::new(refresh_token.clone()))
                .request_async(&http_client)
                .await
                .map_err(|e| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to refresh token: {}", e)}))
                ))?;

            let new_access_token = token_result.access_token().secret();
            let expires_in = token_result.expires_in()
                .unwrap_or_default()
                .as_secs() as i32;

            // Update the access token in the database
            state.user_repository.update_google_tasks_access_token(
                new_access_token.clone().as_str(),
                expires_in,
            ).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to update token: {}", e)}))
            ))?;

            // Retry with new token
            fetch_tasks_with_token(&client, &new_access_token)
                .await
                .map_err(|e| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to fetch tasks after token refresh: {}", e)}))
                ))?
        },
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to fetch tasks: {}", e)}))
            ));
        }
    };


    Ok(Json(json!({
        "tasks": tasks
    })))
}

