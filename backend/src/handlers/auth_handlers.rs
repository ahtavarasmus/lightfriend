use std::sync::Arc;
use axum::{
    Json,
    extract::State,
    response::Response,
    http::{StatusCode, HeaderMap}
};
use serde_json::json;
use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation, Algorithm};
use chrono::{Duration, Utc};

use crate::{
    handlers::auth_dtos::{LoginRequest, RegisterRequest, RegisterResponse, UserResponse, Claims, NewUser},
    AppState
};


pub async fn get_users(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<UserResponse>>, (StatusCode, Json<serde_json::Value>)> {
    // Extract token from Authorization header
    let auth_header = headers.get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(token) => token,
        None => return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "No authorization token provided"}))
        )),
    };

    // Decode and validate JWT token
    let claims = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(std::env::var("JWT_SECRET_KEY")
                    .expect("JWT_SECRET_KEY must be set in environment")
                    .as_bytes()),
        &Validation::new(Algorithm::HS256)
    ) {
        Ok(token_data) => token_data.claims,
        Err(_) => return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid token"}))
        )),
    };
    // Check if the user is admin
    if !state.user_repository.is_admin(claims.sub).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))? {
        return Err((
            StatusCode::FORBIDDEN, 
            Json(json!({"error": "Only admin can access this endpoint"}))
        ));
    }

    let users_list = state.user_repository.get_all_users().map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
    let users_response: Vec<UserResponse> = users_list
        .into_iter()
        .map(|(id, username, email)| UserResponse { id, username, email })
        .collect();

    Ok(Json(users_response))
}


pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(login_req): Json<LoginRequest>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    println!("Login attempt for username: {}", login_req.username); // Debug log

    let user = match state.user_repository.find_by_username(&login_req.username) {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid credentials"}))
            ));
        }
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"}))
            ));
        }
    };
   
    match bcrypt::verify(&login_req.password, &user.password_hash) {
        Ok(valid) => {
            println!("Password verification result: {}", valid);
            if valid {
                println!("Password verified successfully, generating tokens");
                // Generate access token (short-lived)
                let access_token = encode(
                    &Header::default(),
                    &json!({
                        "sub": user.id,
                        "exp": (Utc::now() + Duration::minutes(15)).timestamp(),
                        "type": "access"
                    }),
                    &EncodingKey::from_secret(std::env::var("JWT_SECRET_KEY")
                        .expect("JWT_SECRET_KEY must be set in environment")
                        .as_bytes()),
                ).map_err(|_| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Token generation failed"}))
                ))?;
                println!("Access token generated successfully");

                // Generate refresh token (long-lived)
                let refresh_token = encode(
                    &Header::default(),
                    &json!({
                        "sub": user.id,
                        "exp": (Utc::now() + Duration::days(7)).timestamp(),
                        "type": "refresh"
                    }),
                    &EncodingKey::from_secret(std::env::var("JWT_REFRESH_KEY")
                        .expect("JWT_REFRESH_KEY must be set in environment")
                        .as_bytes()),
                ).map_err(|_| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Token generation failed"}))
                ))?;
                println!("Refresh token generated successfully");
                
                // Create response with HttpOnly cookies
                let mut response = Response::new(
                    axum::body::boxed(axum::body::Full::from(
                        Json(json!({"message": "Login successful", "token": access_token})).to_string()
                    ))
                );
                println!("Created base response");
                
                let cookie_options = "; HttpOnly; Secure; SameSite=Strict; Path=/";
                response.headers_mut().insert(
                    "Set-Cookie",
                    format!("access_token={}{}; Max-Age=900", access_token, cookie_options)
                        .parse()
                        .unwrap(),
                );
                println!("Added access token cookie");
                
                response.headers_mut().insert(
                    "Set-Cookie",
                    format!("refresh_token={}{}; Max-Age=604800", refresh_token, cookie_options)
                        .parse()
                        .unwrap(),
                );
                println!("Added refresh token cookie");
                
                // Set content type header
                response.headers_mut().insert(
                    "Content-Type",
                    "application/json".parse().unwrap()
                );
                println!("Added content type header");

                println!("Returning successful response");
                Ok(response)
            } else {
                println!("Password verification failed");
                Err((
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error": "Invalid credentials"}))
                ))
            }
        }
        Err(err) => {
            println!("Password verification error occurred: {:?}", err);
            Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid credentials"}))
            ))
        }
    }
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(reg_req): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, (StatusCode, Json<serde_json::Value>)> {
    
    println!("Registration attempt for username: {}", reg_req.username);

    // Check if username exists
    println!("Checking if username exists...");
    if state.user_repository.username_exists(&reg_req.username).map_err(|e| {
        println!("Database error while checking username: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR, 
            Json(json!({ "error": format!("Database error: {}", e) }))
        )
    })? {
        println!("Username {} already exists", reg_req.username);
        return Err((
            StatusCode::CONFLICT,
            Json(json!({ "error": "Username already exists" })),
        ));
    }
    println!("Username is available");

    // Hash password
    println!("Hashing password...");
    let password_hash = bcrypt::hash(&reg_req.password, bcrypt::DEFAULT_COST)
        .map_err(|e| {
            println!("Password hashing failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Password hashing failed: {}", e) })),
            )
        })?;
    println!("Password hashed successfully");

    // Create and insert user
    println!("Creating new user...");
    let new_user = NewUser {
        username: reg_req.username,
        password_hash: password_hash,
        email: reg_req.email,
    };

    state.user_repository.create_user(new_user).map_err(|e| {
        println!("User creation failed: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("User creation failed: {}", e) })),
        )
    })?;

    println!("User registered successfully");
    Ok(Json(RegisterResponse {
        message: "User registered successfully! Redirecting...".to_string(),
    }))
}
