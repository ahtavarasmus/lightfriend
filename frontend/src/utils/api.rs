use crate::config;
use gloo_net::http::Request;
use gloo_net::Error as GlooError;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use web_sys::RequestCredentials;

// Global flag to prevent redirect loops
static REDIRECTING_TO_LOGIN: AtomicBool = AtomicBool::new(false);

/// Centralized API client with automatic authentication handling and token refresh
pub struct Api;

/// Request wrapper that provides automatic token refresh and retry on 401
pub struct RequestWrapper {
    request: Request,
    path: String,
    method: String,
    body_data: Option<String>,
}

impl RequestWrapper {
    /// Create a new RequestWrapper
    fn new(path: &str, method: &str) -> Self {
        let full_url = format!("{}{}", config::get_backend_url(), path);
        let request = match method {
            "POST" => Request::post(&full_url),
            "DELETE" => Request::delete(&full_url),
            "PATCH" => Request::patch(&full_url),
            "PUT" => Request::put(&full_url),
            _ => Request::get(&full_url),
        }
        .credentials(RequestCredentials::Include);

        Self {
            request,
            path: path.to_string(),
            method: method.to_string(),
            body_data: None,
        }
    }

    /// Add a header to the request
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.request = self.request.header(name, value);
        self
    }

    /// Set the request body
    pub fn body(mut self, body: impl Into<String>) -> Self {
        let body_string = body.into();
        self.request = self.request.body(body_string.clone());
        self.body_data = Some(body_string);
        self
    }

    /// Set the request body as JSON
    pub fn json<T: Serialize>(mut self, data: &T) -> Result<Self, serde_json::Error> {
        let body_string = serde_json::to_string(data)?;
        self.request = self.request.header("Content-Type", "application/json");
        self.request = self.request.body(body_string.clone());
        self.body_data = Some(body_string);
        Ok(self)
    }

    /// Send the request with automatic token refresh and retry on 401
    pub async fn send(self) -> Result<gloo_net::http::Response, GlooError> {
        // Check if we're already redirecting to login
        if REDIRECTING_TO_LOGIN.load(Ordering::Relaxed) {
            gloo_console::log!("Already redirecting to login, skipping request");
            return Err(GlooError::GlooError("Redirecting to login".to_string()));
        }

        // Send the initial request
        let response = self.request.send().await?;

        // Check if we got a 401
        if response.status() == 401 {
            // Special case: if this is an auth status check, try refresh but never redirect
            if self.path == "/api/auth/status" {
                gloo_console::log!("Auth status check returned 401, attempting token refresh...");
                let refresh_url = format!("{}/api/auth/refresh", config::get_backend_url());
                if let Ok(refresh_resp) = Request::post(&refresh_url)
                    .credentials(RequestCredentials::Include)
                    .send()
                    .await
                {
                    if refresh_resp.ok() {
                        gloo_console::log!(
                            "Token refresh succeeded, retrying auth status check..."
                        );
                        let full_url = format!("{}/api/auth/status", config::get_backend_url());
                        if let Ok(retry) = Request::get(&full_url)
                            .credentials(RequestCredentials::Include)
                            .send()
                            .await
                        {
                            return Ok(retry);
                        }
                    }
                }
                gloo_console::log!("Token refresh failed, user is not logged in");
                return Ok(response);
            }

            // Check again if we're already redirecting (race condition protection)
            if REDIRECTING_TO_LOGIN.load(Ordering::Relaxed) {
                gloo_console::log!("Already redirecting to login, skipping refresh attempt");
                return Ok(response);
            }

            gloo_console::log!("Got 401, attempting token refresh...");

            // Try to refresh the token
            let refresh_result =
                Request::post(&format!("{}/api/auth/refresh", config::get_backend_url()))
                    .credentials(RequestCredentials::Include)
                    .send()
                    .await;

            match refresh_result {
                Ok(refresh_resp) if refresh_resp.ok() => {
                    gloo_console::log!("Token refresh successful, retrying original request...");

                    // Recreate and retry the original request
                    let full_url = format!("{}{}", config::get_backend_url(), self.path);
                    let mut retry_request = match self.method.as_str() {
                        "POST" => Request::post(&full_url),
                        "DELETE" => Request::delete(&full_url),
                        "PATCH" => Request::patch(&full_url),
                        "PUT" => Request::put(&full_url),
                        _ => Request::get(&full_url),
                    }
                    .credentials(RequestCredentials::Include);

                    // Re-add body and Content-Type header if present
                    if let Some(body) = self.body_data {
                        retry_request = retry_request
                            .header("Content-Type", "application/json")
                            .body(body);
                    }

                    let retry_response = retry_request.send().await?;

                    // If retry also returns 401, refresh token is invalid - redirect to login
                    if retry_response.status() == 401 {
                        gloo_console::log!(
                            "Retry also returned 401, refresh token invalid, redirecting to login"
                        );
                        REDIRECTING_TO_LOGIN.store(true, Ordering::Relaxed);
                        if let Some(window) = web_sys::window() {
                            let _ = window.location().set_href("/login");
                        }
                    } else {
                        gloo_console::log!("Retry successful!");
                    }

                    Ok(retry_response)
                }
                _ => {
                    // Token refresh failed - session is expired, redirect to login
                    gloo_console::log!("Token refresh failed, redirecting to login");
                    REDIRECTING_TO_LOGIN.store(true, Ordering::Relaxed);
                    if let Some(window) = web_sys::window() {
                        let _ = window.location().set_href("/login");
                    }
                    Ok(response)
                }
            }
        } else {
            // Not a 401, return response as-is
            Ok(response)
        }
    }
}

impl Api {
    /// Create a GET request with automatic credentials and backend URL
    pub fn get(path: &str) -> RequestWrapper {
        RequestWrapper::new(path, "GET")
    }

    /// Create a POST request with automatic credentials and backend URL
    pub fn post(path: &str) -> RequestWrapper {
        RequestWrapper::new(path, "POST")
    }

    /// Create a DELETE request with automatic credentials and backend URL
    pub fn delete(path: &str) -> RequestWrapper {
        RequestWrapper::new(path, "DELETE")
    }

    /// Create a PATCH request with automatic credentials and backend URL
    pub fn patch(path: &str) -> RequestWrapper {
        RequestWrapper::new(path, "PATCH")
    }

    /// Create a PUT request with automatic credentials and backend URL
    pub fn put(path: &str) -> RequestWrapper {
        RequestWrapper::new(path, "PUT")
    }
}
