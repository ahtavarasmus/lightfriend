/*
use crate::models::user_models::{Idea, NewIdea, IdeaUpvote, NewIdeaUpvote, IdeaEmailSubscription, NewIdeaEmailSubscription};
use crate::error::Error;
use axum::{
    extract::{Path, State},
    Json,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;
use std::sync::Arc;
use crate::AppState;
use std::time::{SystemTime, UNIX_EPOCH};
use diesel::result::Error as DieselError;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateIdeaRequest {
    #[validate(length(min = 1, message = "Text cannot be empty"))]
    text: String,
}

#[derive(Debug, Serialize)]
pub struct IdeaResponse {
    id: Option<i32>,
    text: String,
    created_at: i32,
    is_upvoted: bool,
}

#[derive(Debug, Deserialize, Validate)]
pub struct EmailSubscriptionRequest {
    #[validate(email(message = "Invalid email format"))]
    email: String,
}

impl CreateIdeaRequest {
    pub fn validate(&self) -> Result<(), Error> {
        <Self as Validate>::validate(self)
            .map_err(|e| Error::ValidationError(e))
    }
}

impl EmailSubscriptionRequest {
    pub fn validate(&self) -> Result<(), Error> {
        <Self as Validate>::validate(self)
            .map_err(|e| Error::ValidationError(e))
    }
}

pub async fn create_idea(
    State(state): State<Arc<AppState>>,
    user_id: String, // From auth middleware
    Json(req): Json<CreateIdeaRequest>,
) -> Result<Json<IdeaResponse>, Error> {
    req.validate()?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let new_idea = NewIdea {
        creator_id: user_id,
        text: req.text,
        created_at: now,
    };

    let idea = diesel::insert_into(crate::schema::ideas::table)
        .values(&new_idea)
        .execute(&mut state.db_pool.get()?)?;

    let idea = crate::schema::ideas::table
        .order(crate::schema::ideas::id.desc())
        .first::<Idea>(&mut state.db_pool.get()?)?;

    Ok(Json(IdeaResponse {
        id: idea.id,
        text: idea.text,
        created_at: idea.created_at,
        is_upvoted: false,
    }))
}

pub async fn get_ideas(
    State(state): State<Arc<AppState>>,
    user_id: String, // From auth middleware
) -> Result<Json<Vec<IdeaResponse>>, Error> {
    use crate::schema::ideas::dsl::*;
    use crate::schema::idea_upvotes::dsl as upvotes_dsl;

    let mut conn = state.db_pool.get()?;

    let ideas_with_upvotes = ideas
        .order_by(created_at.desc())
        .load::<Idea>(&mut conn)?;

    let user_upvotes: Vec<i32> = upvotes_dsl::idea_upvotes
        .filter(upvotes_dsl::voter_id.eq(&user_id))
        .select(upvotes_dsl::idea_id)
        .load(&mut conn)?;

    let ideas_response = ideas_with_upvotes
        .into_iter()
        .filter_map(|idea| {
            idea.id.map(|id| IdeaResponse {
                id: Some(id),
                text: idea.text,
                created_at: idea.created_at,
                is_upvoted: user_upvotes.contains(&id),
            })
        })
        .collect();

    Ok(Json(ideas_response))
}

pub async fn upvote_idea(
    State(state): State<Arc<AppState>>,
    Path(idea_id): Path<i32>,
    user_id: String, // From auth middleware
) -> Result<Json<IdeaResponse>, Error> {
    let mut conn = state.db_pool.get()?;

    // Check if already upvoted
    use crate::schema::idea_upvotes::dsl::*;
    let existing_upvote = idea_upvotes
        .filter(voter_id.eq(&user_id))
        .filter(idea_id.eq(idea_id))
        .first::<IdeaUpvote>(&mut conn)
        .optional()?;

    if existing_upvote.is_none() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let new_upvote = NewIdeaUpvote {
            idea_id,
            voter_id: user_id.clone(),
            created_at: now,
        };

        diesel::insert_into(crate::schema::idea_upvotes::table)
            .values(&new_upvote)
            .execute(&mut conn)?;
    }

    // Get updated idea
    use crate::schema::ideas::dsl::*;
    let idea = ideas
        .filter(crate::schema::ideas::id.eq(idea_id))
        .first::<Idea>(&mut conn)?;

    Ok(Json(IdeaResponse {
        id: idea.id.unwrap(),
        text: idea.text,
        created_at: idea.created_at,
        is_upvoted: true,
    }))
}

pub async fn subscribe_email(
    State(state): State<Arc<AppState>>,
    Path(idea_id): Path<i32>,
    user_id: String, // From auth middleware
    Json(req): Json<EmailSubscriptionRequest>,
) -> Result<Json<&'static str>, Error> {
    req.validate()?;

    let mut conn = state.db_pool.get()?;

    // Check if user created or upvoted the idea
    use crate::schema::ideas::dsl::*;
    use crate::schema::idea_upvotes::dsl as upvotes_dsl;

    let is_creator = ideas
        .filter(crate::schema::ideas::id.eq(idea_id))
        .filter(creator_id.eq(&user_id))
        .first::<Idea>(&mut conn)
        .optional()?
        .is_some();

    let has_upvoted = upvotes_dsl::idea_upvotes
        .filter(upvotes_dsl::voter_id.eq(&user_id))
        .filter(upvotes_dsl::idea_id.eq(idea_id))
        .first::<IdeaUpvote>(&mut conn)
        .optional()?
        .is_some();

    if !is_creator && !has_upvoted {
        return Err(Error::BadRequest("Must create or upvote idea to subscribe".into()));
    }

    // Check if already subscribed
    use crate::schema::idea_email_subscriptions::dsl::*;
    let existing_sub = idea_email_subscriptions
        .filter(email.eq(&req.email))
        .filter(idea_id.eq(idea_id))
        .first::<IdeaEmailSubscription>(&mut conn)
        .optional()?;

    if existing_sub.is_none() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let new_sub = NewIdeaEmailSubscription {
            idea_id,
            email: req.email,
            created_at: now,
        };

        diesel::insert_into(crate::schema::idea_email_subscriptions::table)
            .values(&new_sub)
            .execute(&mut conn)?;
    }

    Ok(Json("Email subscribed"))
}
*/
