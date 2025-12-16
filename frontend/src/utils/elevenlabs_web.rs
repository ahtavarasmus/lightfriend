// ElevenLabs Web Call JavaScript interop
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    /// Start an ElevenLabs voice call with the given signed URL and overrides
    /// Returns a promise that resolves to true on success, false on failure
    #[wasm_bindgen(js_name = startElevenLabsCall)]
    pub async fn start_elevenlabs_call(signed_url: &str, overrides: JsValue) -> JsValue;

    /// End the active ElevenLabs call
    /// Returns the call duration in seconds
    #[wasm_bindgen(js_name = endElevenLabsCall)]
    pub async fn end_elevenlabs_call() -> JsValue;

    /// Get the current call duration in seconds
    #[wasm_bindgen(js_name = getElevenLabsCallDuration)]
    pub fn get_elevenlabs_call_duration() -> i32;

    /// Check if a call is currently active
    #[wasm_bindgen(js_name = isElevenLabsCallActive)]
    pub fn is_elevenlabs_call_active() -> bool;

    /// Get the current call status ("disconnected", "connecting", "connected", "error")
    #[wasm_bindgen(js_name = getElevenLabsCallStatus)]
    pub fn get_elevenlabs_call_status() -> String;
}

/// Helper to check if ElevenLabs SDK is loaded
pub fn is_sdk_available() -> bool {
    let window = web_sys::window().expect("no window");
    window.get("startElevenLabsCall").is_some()
}
