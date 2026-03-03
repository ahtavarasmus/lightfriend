use crate::AppState;
use crate::UserCoreOps;
use axum::{
    body::Body,
    extract::{FromRequestParts, State},
    http::{header::HeaderMap, request::Parts, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde_json::json;
use std::sync::Arc;

use crate::handlers::auth_dtos::Claims;

#[derive(Clone, Copy)]
pub struct AuthUser {
    pub user_id: i32,
    pub is_admin: bool,
}

use tracing::{debug, error, info};

// Helper function to check if a tool requires subscription
// Only tier 2 (hosted) subscribers get full access to all tools
fn requires_subscription(path: &str, sub_tier: Option<String>, has_discount: bool) -> bool {
    debug!(
        path = path,
        subscription = ?sub_tier,
        discount = has_discount,
        "Checking subscription access"
    );

    // Only tier 2 (hosted) subscribers and users with discount get full access to everything
    if sub_tier == Some("tier 2".to_string()) || has_discount {
        debug!("User has tier 2 subscription or discount - granting full access");
        return false;
    }

    debug!("Tool requires tier 2 subscription");
    true
}

pub async fn check_subscription_access(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    info!("Starting subscription access check");

    // Extract user_id from query parameters
    let uri = request.uri();
    let query_string = uri.query().unwrap_or("");
    let query_params: std::collections::HashMap<String, String> =
        url::form_urlencoded::parse(query_string.as_bytes())
            .into_owned()
            .collect();

    let user_id = match query_params
        .get("user_id")
        .and_then(|id| id.parse::<i32>().ok())
    {
        Some(id) => {
            debug!("Found user_id in query parameters: {}", id);
            id
        }
        None => {
            error!("No valid user_id found in query parameters");
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid user_id"
                })),
            ));
        }
    };

    // Get user from database
    let user = match state.user_core.find_by_id(user_id) {
        Ok(Some(user)) => {
            debug!("Found user: {}", user.email);
            user
        }
        Ok(None) => {
            error!("User not found: {}", user_id);
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "User not found"
                })),
            ));
        }
        Err(e) => {
            error!("Database error: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Internal server error"
                })),
            ));
        }
    };

    // Check if the tool requires subscription
    if requires_subscription(request.uri().path(), user.sub_tier, user.discount) {
        info!("Tool requires subscription, user doesn't have access");
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "This tool requires a subscription",
                "message": "Please upgrade your subscription to access this feature",
                "upgrade_url": "/billing"
            })),
        ));
    }

    info!("Subscription access check passed");
    Ok(next.run(request).await)
}

// Add this new middleware function for admin routes
pub async fn require_admin(
    State(_state): State<Arc<AppState>>,
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

pub async fn require_auth(request: Request<Body>, next: Next) -> Result<Response, AuthError> {
    // Try Bearer token first, then fall back to cookies
    let token = extract_bearer_token(request.headers())
        .or_else(|| extract_cookie_token(request.headers()))
        .ok_or(AuthError {
            status: StatusCode::UNAUTHORIZED,
            message: "No authorization token provided".to_string(),
        })?;

    // Validate the token
    decode::<Claims>(
        &token,
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

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        // Try Bearer token first, then fall back to cookies
        let token = extract_bearer_token(&parts.headers)
            .or_else(|| extract_cookie_token(&parts.headers))
            .ok_or_else(|| {
                tracing::debug!("No authorization token found in Bearer header or cookies");
                AuthError {
                    status: StatusCode::UNAUTHORIZED,
                    message: "No authorization token provided".to_string(),
                }
            })?;

        // Decode the token
        let claims = decode::<Claims>(
            &token,
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

        // Check if user is admin
        let is_admin = state
            .user_core
            .is_admin(claims.sub)
            .map_err(|_| AuthError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: "Failed to check admin status".to_string(),
            })?;

        Ok(AuthUser {
            user_id: claims.sub,
            is_admin,
        })
    }
}

/// Extract JWT from `Authorization: Bearer <token>` header.
fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|auth| auth.strip_prefix("Bearer "))
        .map(|t| t.to_string())
}

/// Extract JWT from `access_token` cookie.
fn extract_cookie_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').map(|s| s.trim()).find_map(|cookie| {
                let parts: Vec<&str> = cookie.splitn(2, '=').collect();
                if parts.len() == 2 && parts[0] == "access_token" {
                    Some(parts[1].to_string())
                } else {
                    None
                }
            })
        })
}

/// Extract JWT from a query parameter (used for WebSocket connections).
pub fn extract_query_token(uri: &axum::http::Uri) -> Option<String> {
    uri.query().and_then(|q| {
        url::form_urlencoded::parse(q.as_bytes())
            .find(|(k, _)| k == "token")
            .map(|(_, v)| v.to_string())
    })
}
