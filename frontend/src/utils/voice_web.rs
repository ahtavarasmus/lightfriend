// Voice call JavaScript interop
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    /// Start a voice call with the given signed URL and overrides
    #[wasm_bindgen(js_name = startVoiceCall)]
    pub async fn start_voice_call(signed_url: &str, overrides: JsValue) -> JsValue;

    /// End the active voice call
    #[wasm_bindgen(js_name = endVoiceCall)]
    pub async fn end_voice_call() -> JsValue;

    #[wasm_bindgen(js_name = getVoiceCallDuration)]
    pub fn get_voice_call_duration() -> i32;

    #[wasm_bindgen(js_name = isVoiceCallActive)]
    pub fn is_voice_call_active() -> bool;

    #[wasm_bindgen(js_name = getVoiceCallStatus)]
    pub fn get_voice_call_status() -> String;
}
