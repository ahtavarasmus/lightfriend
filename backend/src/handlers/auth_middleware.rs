use futures::Future;
use axum::{
    extract::{FromRequestParts, State},
    http::{Request, request::Parts, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    body::Body,
    Json,
};
use std::sync::Arc;
use crate::AppState;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde_json::json;

use crate::handlers::auth_dtos::Claims;


#[derive(Clone, Copy)]
pub struct AuthUser {
    pub user_id: i32,
    pub is_admin: bool,
}

use tracing::{error, info, debug};


// Add this new middleware function for admin routes
pub async fn require_admin(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    request: Request<Body>,
    next: Next,
) -> Result<Response, AuthError> {
    if !auth_user.is_admin {
        return Err(AuthError {
            status: StatusCode::FORBIDDEN,
            message: "Admin access required".to_string(),
        });
    }
    
    Ok(next.run(request).await)
}

pub async fn require_auth(
    request: Request<Body>,
    next: Next,
) -> Result<Response, AuthError> {
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "));

    let token = auth_header.ok_or(AuthError {
        status: StatusCode::UNAUTHORIZED,
        message: "No authorization token provided".to_string(),
    })?;

    // Validate the token
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(
            std::env::var("JWT_SECRET_KEY")
                .expect("JWT_SECRET_KEY must be set in environment")
                .as_bytes(),
        ),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|_| AuthError {
        status: StatusCode::UNAUTHORIZED,
        message: "Invalid token".to_string(),
    })?;

    Ok(next.run(request).await)
}

#[derive(Debug)]
pub struct AuthError {
    pub status: StatusCode,
    pub message: String,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let body = Json(json!({
            "error": self.message,
        }));
        
        (self.status, body).into_response()
    }
}


impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = AuthError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        async move {
        // Extract the token from the Authorization header
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|header| header.to_str().ok())
            .and_then(|header| header.strip_prefix("Bearer "));

        let token = auth_header.ok_or(AuthError {
            status: StatusCode::UNAUTHORIZED,
            message: "No authorization token provided".to_string(),
        })?;

        // Decode the token
        let claims = decode::<Claims>(
            token,
            &DecodingKey::from_secret(
                std::env::var("JWT_SECRET_KEY")
                    .expect("JWT_SECRET_KEY must be set in environment")
                    .as_bytes(),
            ),
            &Validation::new(Algorithm::HS256),
        )
        .map_err(|_| AuthError {
            status: StatusCode::UNAUTHORIZED,
            message: "Invalid token".to_string(),
        })?
        .claims;

        let is_admin = true;

        Ok(AuthUser {
            user_id: claims.sub,
            is_admin,
        })
        }
    }
}

