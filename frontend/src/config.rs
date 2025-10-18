#[cfg(debug_assertions)]
pub fn get_backend_url() -> &'static str {
    "http://localhost:3001"  // Development URL when running locally
}

#[cfg(not(debug_assertions))]
pub fn get_backend_url() -> &'static str {
    ""  // Production URL
}

