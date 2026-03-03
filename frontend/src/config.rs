#[cfg(debug_assertions)]
pub fn get_backend_url() -> &'static str {
    "http://localhost:3000"  // Development URL when running locally
}

#[cfg(all(not(debug_assertions), feature = "tauri"))]
pub fn get_backend_url() -> &'static str {
    "https://lightfriend.ai"  // Tauri app talks to production API
}

#[cfg(all(not(debug_assertions), not(feature = "tauri")))]
pub fn get_backend_url() -> &'static str {
    ""  // Web production: same-origin, no prefix needed
}

/// Returns true when running inside a Tauri webview
pub fn is_tauri() -> bool {
    cfg!(feature = "tauri")
}
