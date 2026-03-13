use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::{
    handlers::auth_middleware::AuthUser,
    models::ontology_models::PersonWithChannels,
    AppState,
};

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

/// GET /api/persons/search/:service?q=query
/// Searches for chats on the specified service (reuses existing bridge room search)
pub async fn search_chats(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(service): Path<String>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Validate service
    if !["whatsapp", "telegram", "signal"].contains(&service.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                json!({ "error": "Invalid service. Must be 'whatsapp', 'telegram', or 'signal'" }),
            ),
        ));
    }

    // Use existing bridge room search functionality
    match crate::utils::bridge::search_bridge_rooms(&service, &state, auth_user.user_id, &query.q)
        .await
    {
        Ok(rooms) => {
            // Look up which rooms are already assigned to persons via ontology
            let room_ids: Vec<String> = rooms
                .iter()
                .filter(|r| !r.room_id.is_empty())
                .map(|r| r.room_id.clone())
                .collect();

            let assigned_rooms = if !room_ids.is_empty() {
                state
                    .ontology_repository
                    .find_channels_by_room_ids(
                        auth_user.user_id,
                        &room_ids,
                        None,
                    )
                    .unwrap_or_default()
            } else {
                std::collections::HashMap::new()
            };

            let results: Vec<serde_json::Value> = rooms
                .iter()
                .map(|room| {
                    let person_name = if !room.room_id.is_empty() {
                        assigned_rooms.get(&room.room_id).cloned()
                    } else {
                        None
                    };
                    json!({
                        "display_name": room.display_name,
                        "last_activity_formatted": room.last_activity_formatted,
                        "room_id": room.room_id,
                        "is_group": room.is_group,
                        "person_name": person_name,
                        "is_phone_contact": crate::utils::bridge::is_phone_contact_from_room_name(&room.display_name)
                    })
                })
                .collect();

            Ok(Json(json!({ "results": results })))
        }
        Err(e) => {
            tracing::error!("Failed to search chats: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to search: {}", e) })),
            ))
        }
    }
}

// --- Person + Channel (Ontology) Handlers ---

#[derive(Deserialize)]
pub struct CreatePersonRequest {
    pub name: String,
    pub channels: Option<Vec<CreateChannelRequest>>,
}

#[derive(Deserialize)]
pub struct CreateChannelRequest {
    pub platform: String,
    pub handle: Option<String>,
    pub room_id: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdatePersonRequest {
    pub name: Option<String>,
    pub nickname: Option<String>,
    pub notes: Option<String>,
    pub notification_mode: Option<String>,
    pub notification_type: Option<String>,
    pub notify_on_call: Option<bool>,
}

#[derive(Deserialize)]
pub struct UpdateChannelRequest {
    pub notification_mode: Option<String>,
    pub notification_type: Option<String>,
    pub notify_on_call: Option<i32>,
}

#[derive(Deserialize)]
pub struct MergePersonsRequest {
    pub keep_id: i32,
    pub merge_id: i32,
}

pub async fn get_persons(
    State(state): State<Arc<AppState>>,
    Extension(user_id): Extension<i32>,
) -> Result<Json<Vec<PersonWithChannels>>, StatusCode> {
    state.ontology_repository
        .get_persons_with_channels(user_id)
        .map(Json)
        .map_err(|e| {
            tracing::error!("Failed to get persons: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn create_person(
    State(state): State<Arc<AppState>>,
    Extension(user_id): Extension<i32>,
    Json(req): Json<CreatePersonRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let person = state.ontology_repository
        .create_person(user_id, &req.name)
        .map_err(|e| {
            tracing::error!("Failed to create person: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Add channels if provided
    if let Some(channels) = req.channels {
        for ch in channels {
            let _ = state.ontology_repository.add_channel(
                user_id,
                person.id,
                &ch.platform,
                ch.handle.as_deref(),
                ch.room_id.as_deref(),
            );
        }
    }

    // Return full person with channels
    let full = state.ontology_repository
        .get_person_with_channels(user_id, person.id)
        .map_err(|e| {
            tracing::error!("Failed to get created person: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(serde_json::to_value(full).unwrap_or_default()))
}

pub async fn update_person(
    State(state): State<Arc<AppState>>,
    Extension(user_id): Extension<i32>,
    Path(person_id): Path<i32>,
    Json(req): Json<UpdatePersonRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Update base name if provided
    if let Some(ref name) = req.name {
        state.ontology_repository.update_person_name(user_id, person_id, name)
            .map_err(|e| {
                tracing::error!("Failed to update person name: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    }

    // Set edits for each provided override
    if let Some(ref nickname) = req.nickname {
        let _ = state.ontology_repository.set_person_edit(user_id, person_id, "nickname", nickname);
    }
    if let Some(ref notes) = req.notes {
        let _ = state.ontology_repository.set_person_edit(user_id, person_id, "notes", notes);
    }
    if let Some(ref mode) = req.notification_mode {
        let _ = state.ontology_repository.set_person_edit(user_id, person_id, "notification_mode", mode);
    }
    if let Some(ref ntype) = req.notification_type {
        let _ = state.ontology_repository.set_person_edit(user_id, person_id, "notification_type", ntype);
    }
    if let Some(on_call) = req.notify_on_call {
        let val = if on_call { "1" } else { "0" };
        let _ = state.ontology_repository.set_person_edit(user_id, person_id, "notify_on_call", val);
    }

    let full = state.ontology_repository
        .get_person_with_channels(user_id, person_id)
        .map_err(|e| {
            tracing::error!("Failed to get updated person: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(serde_json::to_value(full).unwrap_or_default()))
}

pub async fn delete_person(
    State(state): State<Arc<AppState>>,
    Extension(user_id): Extension<i32>,
    Path(person_id): Path<i32>,
) -> Result<StatusCode, StatusCode> {
    state.ontology_repository.delete_person(user_id, person_id)
        .map_err(|e| {
            tracing::error!("Failed to delete person: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn add_person_channel(
    State(state): State<Arc<AppState>>,
    Extension(user_id): Extension<i32>,
    Path(person_id): Path<i32>,
    Json(req): Json<CreateChannelRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let channel = state.ontology_repository.add_channel(
        user_id, person_id, &req.platform, req.handle.as_deref(), req.room_id.as_deref(),
    ).map_err(|e| {
        tracing::error!("Failed to add channel: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(serde_json::to_value(channel).unwrap_or_default()))
}

pub async fn update_person_channel(
    State(state): State<Arc<AppState>>,
    Extension(user_id): Extension<i32>,
    Path((person_id, channel_id)): Path<(i32, i32)>,
    Json(req): Json<UpdateChannelRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let _ = person_id; // Validated by ownership check below
    let mode = req.notification_mode.as_deref().unwrap_or("default");
    let ntype = req.notification_type.as_deref().unwrap_or("sms");
    let on_call = req.notify_on_call.unwrap_or(1);

    state.ontology_repository.update_channel_notification(channel_id, mode, ntype, on_call)
        .map_err(|e| {
            tracing::error!("Failed to update channel: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Return updated person
    let full = state.ontology_repository
        .get_person_with_channels(user_id, person_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::to_value(full).unwrap_or_default()))
}

pub async fn delete_person_channel(
    State(state): State<Arc<AppState>>,
    Extension(user_id): Extension<i32>,
    Path((_person_id, channel_id)): Path<(i32, i32)>,
) -> Result<StatusCode, StatusCode> {
    state.ontology_repository.delete_channel(user_id, channel_id)
        .map_err(|e| {
            tracing::error!("Failed to delete channel: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn merge_persons(
    State(state): State<Arc<AppState>>,
    Extension(user_id): Extension<i32>,
    Json(req): Json<MergePersonsRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    state.ontology_repository.merge_persons(user_id, req.keep_id, req.merge_id)
        .map_err(|e| {
            tracing::error!("Failed to merge persons: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let full = state.ontology_repository
        .get_person_with_channels(user_id, req.keep_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::to_value(full).unwrap_or_default()))
}
