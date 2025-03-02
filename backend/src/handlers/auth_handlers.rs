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
use serde::Deserialize;
use crate::api::twilio_sms;
use crate::config::phone_numbers;

#[derive(Deserialize)]
pub struct BroadcastMessageRequest {
    message: String,
}

use crate::{
    handlers::auth_dtos::{LoginRequest, RegisterRequest, RegisterResponse, UserResponse, Claims, NewUser},
    AppState
};

pub async fn get_users(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<UserResponse>>, (StatusCode, Json<serde_json::Value>)> {
    println!("Attempting to get all users");
    
    // Extract token from Authorization header
    let auth_header = headers.get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(token) => {
            println!("Authorization token found");
            token
        },
        None => {
            println!("No authorization token provided");
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "No authorization token provided"}))
            ))
        },
    };

    // Decode and validate JWT token
    println!("Decoding and validating JWT token");
    let claims = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(std::env::var("JWT_SECRET_KEY")
                    .expect("JWT_SECRET_KEY must be set in environment")
                    .as_bytes()),
        &Validation::new(Algorithm::HS256)
    ) {
        Ok(token_data) => {
            println!("Token successfully decoded");
            token_data.claims
        },
        Err(_) => {
            println!("Invalid token");
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid token"}))
            ))
        },
    };
    
    // Check if the user is admin
    println!("Checking admin status for user ID: {}", claims.sub);
    if !state.user_repository.is_admin(claims.sub).map_err(|e| {
        println!("Database error while checking admin status: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
        )
    })? {
        println!("User is not an admin");
        return Err((
            StatusCode::FORBIDDEN, 
            Json(json!({"error": "Only admin can access this endpoint"}))
        ));
    }
    println!("Admin status confirmed");

    println!("Fetching all users from database");
    let users_list = state.user_repository.get_all_users().map_err(|e| {
        println!("Database error while fetching users: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
        )
    })?;
    
    println!("Converting users to response format");
    let users_response: Vec<UserResponse> = users_list
        .into_iter()
        .map(|user| UserResponse {
            id: user.id,
            email: user.email,
            phone_number: user.phone_number,
            nickname: user.nickname,
            time_to_live: user.time_to_live,
            verified: user.verified,
            credits: user.credits,
            notify: user.notify,
            preferred_number: user.preferred_number,
        })
        .collect();

    println!("Successfully retrieved {} users", users_response.len());
    Ok(Json(users_response))
}


pub async fn set_preferred_number_default(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::extract::Path(user_id): axum::extract::Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
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
            Json(json!({"error": "Only admins can set preferred numbers"}))
        ));
    }

    // Get the user's phone number
    let user = state.user_repository.find_by_id(user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
        ))?
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"}))
        ))?;

    // Set the preferred number
    match state.user_repository.set_preferred_number_to_default(user_id, &user.phone_number) {
        Ok(preferred_number) => Ok(Json(json!({
            "message": "Preferred number set successfully",
            "preferred_number": preferred_number
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to set preferred number: {}", e)}))
        )),
    }
}


pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(login_req): Json<LoginRequest>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    println!("Login attempt for email: {}", login_req.email); // Debug log

    let user = match state.user_repository.find_by_email(&login_req.email) {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "User not found"}))
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
                    axum::body::Body::from(
                        Json(json!({"message": "Login successful", "token": access_token})).to_string()
                    )
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
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    
    println!("Registration attempt for email: {}", reg_req.email);

    // Check if email exists
    println!("Checking if email exists...");
    if state.user_repository.email_exists(&reg_req.email).map_err(|e| {
        println!("Database error while checking email: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR, 
            Json(json!({ "error": format!("Database error: {}", e) }))
        )
    })? {
        println!("Email {} already exists", reg_req.email);
        return Err((
            StatusCode::CONFLICT,
            Json(json!({ "error": "Email already exists" })),
        ));
    }
    println!("Email is available");

    // Validate phone number format
    if !reg_req.phone_number.starts_with('+') {
        println!("Invalid phone number format: {}", reg_req.phone_number);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Phone number must start with '+'" })),
        ));
    }

    // Check if phone number exists
    println!("Checking if phone number exists...");
    if state.user_repository.phone_number_exists(&reg_req.phone_number).map_err(|e| {
        println!("Database error while checking phone number: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR, 
            Json(json!({ "error": format!("Database error: {}", e) }))
        )
    })? {
        println!("Phone number {} already exists", reg_req.phone_number);
        return Err((
            StatusCode::CONFLICT,
            Json(json!({ "error": "Phone number already registered" })),
        ));
    }
    println!("Phone number is available");

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
    // Calculate timestamp 5 minutes from now
    let five_minutes_from_now = Utc::now()
        .checked_add_signed(Duration::minutes(5))
        .expect("Failed to calculate timestamp")
        .timestamp() as i32;
    println!("Set the time to live due in 5 minutes");

    let reg_r = reg_req.clone();

    let new_user = NewUser {
        email: reg_r.email,
        password_hash,
        phone_number: reg_r.phone_number,
        time_to_live: five_minutes_from_now,
        notify: true,
        debug_logging_permission: false,
        verified: false,
        credits: 2.00,
        charge_when_under: false,
    };

    state.user_repository.create_user(new_user).map_err(|e| {
        println!("User creation failed: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("User creation failed: {}", e) })),
        )
    })?;

    println!("User registered successfully, setting preferred number");
    
    // Get the newly created user to get their ID
    let user = state.user_repository.find_by_email(&reg_req.email)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to retrieve user: {}", e)}))
        ))?
        .ok_or_else(|| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "User not found after registration"}))
        ))?;

    // Set preferred number
    state.user_repository.set_preferred_number_to_default(user.id, &reg_req.phone_number)
        .map_err(|e| {
            println!("Failed to set preferred number: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to set preferred number: {}", e) })),
            )
        })?;

    println!("Preferred number set successfully, generating tokens");
    let user = state.user_repository.find_by_email(&reg_req.email)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to retrieve user: {}", e)}))
        ))?
        .ok_or_else(|| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "User not found after registration"}))
        ))?;

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

    // Create response with HttpOnly cookies
    let mut response = Response::new(
        axum::body::Body::from(
            Json(json!({
                "message": "User registered and logged in successfully! Redirecting...",
                "token": access_token
            })).to_string()
        )
    );

    let cookie_options = "; HttpOnly; Secure; SameSite=Strict; Path=/";
    response.headers_mut().insert(
        "Set-Cookie",
        format!("access_token={}{}; Max-Age=900", access_token, cookie_options)
            .parse()
            .unwrap(),
    );

    response.headers_mut().insert(
        "Set-Cookie",
        format!("refresh_token={}{}; Max-Age=604800", refresh_token, cookie_options)
            .parse()
            .unwrap(),
    );

    response.headers_mut().insert(
        "Content-Type",
        "application/json".parse().unwrap()
    );

    println!("Registration and login completed successfully");
    Ok(response)
}
