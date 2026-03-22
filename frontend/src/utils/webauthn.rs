use js_sys::{Object, Promise, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::PublicKeyCredential;

// JavaScript interop for WebAuthn
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["navigator", "credentials"], js_name = create)]
    fn credentials_create(options: &JsValue) -> Promise;

    #[wasm_bindgen(js_namespace = ["navigator", "credentials"], js_name = get)]
    fn credentials_get(options: &JsValue) -> Promise;
}

/// Check if WebAuthn is supported in this browser
pub fn is_webauthn_supported() -> bool {
    if let Some(window) = web_sys::window() {
        let navigator = window.navigator();
        // Check if credentials exists on navigator
        let credentials = Reflect::get(&navigator, &JsValue::from_str("credentials"))
            .ok()
            .filter(|v| !v.is_undefined() && !v.is_null());

        if credentials.is_some() {
            // Check if PublicKeyCredential exists
            let pk_cred = Reflect::get(&window, &JsValue::from_str("PublicKeyCredential"))
                .ok()
                .filter(|v| !v.is_undefined() && !v.is_null());
            return pk_cred.is_some();
        }
    }
    false
}

/// Convert a base64url string to Uint8Array
pub fn base64url_to_array(base64url: &str) -> Result<Uint8Array, JsValue> {
    // Replace base64url characters with base64 characters
    let base64 = base64url.replace('-', "+").replace('_', "/");

    // Add padding if needed
    let padding = match base64.len() % 4 {
        0 => "".to_string(),
        2 => "==".to_string(),
        3 => "=".to_string(),
        _ => return Err(JsValue::from_str("Invalid base64url string")),
    };
    let padded = format!("{}{}", base64, padding);

    // Decode base64
    let decoded = web_sys::window()
        .ok_or_else(|| JsValue::from_str("No window"))?
        .atob(&padded)
        .map_err(|_| JsValue::from_str("Failed to decode base64"))?;

    let bytes: Vec<u8> = decoded.chars().map(|c| c as u8).collect();
    let array = Uint8Array::new_with_length(bytes.len() as u32);
    array.copy_from(&bytes);

    Ok(array)
}

/// Convert Uint8Array to base64url string
pub fn array_to_base64url(array: &Uint8Array) -> String {
    let bytes: Vec<u8> = array.to_vec();
    let base64 = base64_encode(&bytes);
    // Convert to base64url
    base64
        .replace('+', "-")
        .replace('/', "_")
        .trim_end_matches('=')
        .to_string()
}

/// Simple base64 encoding
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();

    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;

        result.push(ALPHABET[b0 >> 2] as char);
        result.push(ALPHABET[((b0 & 0x03) << 4) | (b1 >> 4)] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(ALPHABET[b2 & 0x3f] as char);
        } else {
            result.push('=');
        }
    }

    result
}

/// Create a credential (registration)
/// Takes the options JSON from the backend and calls navigator.credentials.create()
pub async fn create_credential(
    options_json: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    if !is_webauthn_supported() {
        return Err("WebAuthn not supported".to_string());
    }

    // Build the options object
    let options = build_creation_options(options_json)?;

    // Call navigator.credentials.create()
    let promise = credentials_create(&options);
    let result = JsFuture::from(promise)
        .await
        .map_err(|e| format!("Credential creation failed: {:?}", e))?;

    // Convert the result to our response format
    let credential: PublicKeyCredential = result
        .dyn_into()
        .map_err(|_| "Failed to cast to PublicKeyCredential".to_string())?;

    serialize_attestation_response(&credential)
}

/// Get a credential (authentication)
/// Takes the options JSON from the backend and calls navigator.credentials.get()
pub async fn get_credential(options_json: &serde_json::Value) -> Result<serde_json::Value, String> {
    if !is_webauthn_supported() {
        return Err("WebAuthn not supported".to_string());
    }

    // Build the options object
    let options = build_request_options(options_json)?;

    // Call navigator.credentials.get()
    let promise = credentials_get(&options);
    let result = JsFuture::from(promise)
        .await
        .map_err(|e| format!("Credential get failed: {:?}", e))?;

    // Convert the result to our response format
    let credential: PublicKeyCredential = result
        .dyn_into()
        .map_err(|_| "Failed to cast to PublicKeyCredential".to_string())?;

    serialize_assertion_response(&credential)
}

/// Build credential creation options object from JSON
fn build_creation_options(json: &serde_json::Value) -> Result<JsValue, String> {
    let options = Object::new();
    let public_key = Object::new();

    let pk_json = json.get("publicKey").ok_or("Missing publicKey")?;

    // Set challenge
    if let Some(challenge) = pk_json.get("challenge").and_then(|c| c.as_str()) {
        let challenge_array = base64url_to_array(challenge)
            .map_err(|e| format!("Failed to decode challenge: {:?}", e))?;
        Reflect::set(&public_key, &"challenge".into(), &challenge_array)
            .map_err(|_| "Failed to set challenge")?;
    }

    // Set rp (relying party)
    if let Some(rp) = pk_json.get("rp") {
        let rp_obj = Object::new();
        if let Some(name) = rp.get("name").and_then(|n| n.as_str()) {
            Reflect::set(&rp_obj, &"name".into(), &JsValue::from_str(name))
                .map_err(|_| "Failed to set rp.name")?;
        }
        if let Some(id) = rp.get("id").and_then(|i| i.as_str()) {
            Reflect::set(&rp_obj, &"id".into(), &JsValue::from_str(id))
                .map_err(|_| "Failed to set rp.id")?;
        }
        Reflect::set(&public_key, &"rp".into(), &rp_obj).map_err(|_| "Failed to set rp")?;
    }

    // Set user
    if let Some(user) = pk_json.get("user") {
        let user_obj = Object::new();
        if let Some(id) = user.get("id").and_then(|i| i.as_str()) {
            let id_array =
                base64url_to_array(id).map_err(|e| format!("Failed to decode user id: {:?}", e))?;
            Reflect::set(&user_obj, &"id".into(), &id_array)
                .map_err(|_| "Failed to set user.id")?;
        }
        if let Some(name) = user.get("name").and_then(|n| n.as_str()) {
            Reflect::set(&user_obj, &"name".into(), &JsValue::from_str(name))
                .map_err(|_| "Failed to set user.name")?;
        }
        if let Some(display_name) = user.get("displayName").and_then(|d| d.as_str()) {
            Reflect::set(
                &user_obj,
                &"displayName".into(),
                &JsValue::from_str(display_name),
            )
            .map_err(|_| "Failed to set user.displayName")?;
        }
        Reflect::set(&public_key, &"user".into(), &user_obj).map_err(|_| "Failed to set user")?;
    }

    // Set pubKeyCredParams
    if let Some(params) = pk_json.get("pubKeyCredParams").and_then(|p| p.as_array()) {
        let params_array = js_sys::Array::new();
        for param in params {
            let param_obj = Object::new();
            if let Some(alg) = param.get("alg").and_then(|a| a.as_i64()) {
                Reflect::set(&param_obj, &"alg".into(), &JsValue::from_f64(alg as f64))
                    .map_err(|_| "Failed to set alg")?;
            }
            if let Some(type_str) = param.get("type").and_then(|t| t.as_str()) {
                Reflect::set(&param_obj, &"type".into(), &JsValue::from_str(type_str))
                    .map_err(|_| "Failed to set type")?;
            }
            params_array.push(&param_obj);
        }
        Reflect::set(&public_key, &"pubKeyCredParams".into(), &params_array)
            .map_err(|_| "Failed to set pubKeyCredParams")?;
    }

    // Set timeout
    if let Some(timeout) = pk_json.get("timeout").and_then(|t| t.as_u64()) {
        Reflect::set(
            &public_key,
            &"timeout".into(),
            &JsValue::from_f64(timeout as f64),
        )
        .map_err(|_| "Failed to set timeout")?;
    }

    // Set attestation
    if let Some(attestation) = pk_json.get("attestation").and_then(|a| a.as_str()) {
        Reflect::set(
            &public_key,
            &"attestation".into(),
            &JsValue::from_str(attestation),
        )
        .map_err(|_| "Failed to set attestation")?;
    }

    // Set authenticatorSelection
    if let Some(auth_sel) = pk_json.get("authenticatorSelection") {
        let auth_sel_obj = Object::new();
        if let Some(attachment) = auth_sel
            .get("authenticatorAttachment")
            .and_then(|a| a.as_str())
        {
            Reflect::set(
                &auth_sel_obj,
                &"authenticatorAttachment".into(),
                &JsValue::from_str(attachment),
            )
            .map_err(|_| "Failed to set authenticatorAttachment")?;
        }
        if let Some(resident_key) = auth_sel.get("residentKey").and_then(|r| r.as_str()) {
            Reflect::set(
                &auth_sel_obj,
                &"residentKey".into(),
                &JsValue::from_str(resident_key),
            )
            .map_err(|_| "Failed to set residentKey")?;
        }
        if let Some(user_verification) = auth_sel.get("userVerification").and_then(|u| u.as_str()) {
            Reflect::set(
                &auth_sel_obj,
                &"userVerification".into(),
                &JsValue::from_str(user_verification),
            )
            .map_err(|_| "Failed to set userVerification")?;
        }
        Reflect::set(&public_key, &"authenticatorSelection".into(), &auth_sel_obj)
            .map_err(|_| "Failed to set authenticatorSelection")?;
    }

    // Set excludeCredentials
    if let Some(exclude) = pk_json.get("excludeCredentials").and_then(|e| e.as_array()) {
        let exclude_array = js_sys::Array::new();
        for cred in exclude {
            let cred_obj = Object::new();
            if let Some(id) = cred.get("id").and_then(|i| i.as_str()) {
                let id_array = base64url_to_array(id)
                    .map_err(|e| format!("Failed to decode exclude credential id: {:?}", e))?;
                Reflect::set(&cred_obj, &"id".into(), &id_array)
                    .map_err(|_| "Failed to set exclude id")?;
            }
            if let Some(type_str) = cred.get("type").and_then(|t| t.as_str()) {
                Reflect::set(&cred_obj, &"type".into(), &JsValue::from_str(type_str))
                    .map_err(|_| "Failed to set exclude type")?;
            }
            exclude_array.push(&cred_obj);
        }
        Reflect::set(&public_key, &"excludeCredentials".into(), &exclude_array)
            .map_err(|_| "Failed to set excludeCredentials")?;
    }

    Reflect::set(&options, &"publicKey".into(), &public_key)
        .map_err(|_| "Failed to set publicKey")?;

    Ok(options.into())
}

/// Build credential request options object from JSON
fn build_request_options(json: &serde_json::Value) -> Result<JsValue, String> {
    let options = Object::new();
    let public_key = Object::new();

    let pk_json = json.get("publicKey").ok_or("Missing publicKey")?;

    // Set challenge
    if let Some(challenge) = pk_json.get("challenge").and_then(|c| c.as_str()) {
        let challenge_array = base64url_to_array(challenge)
            .map_err(|e| format!("Failed to decode challenge: {:?}", e))?;
        Reflect::set(&public_key, &"challenge".into(), &challenge_array)
            .map_err(|_| "Failed to set challenge")?;
    }

    // Set timeout
    if let Some(timeout) = pk_json.get("timeout").and_then(|t| t.as_u64()) {
        Reflect::set(
            &public_key,
            &"timeout".into(),
            &JsValue::from_f64(timeout as f64),
        )
        .map_err(|_| "Failed to set timeout")?;
    }

    // Set rpId
    if let Some(rp_id) = pk_json.get("rpId").and_then(|r| r.as_str()) {
        Reflect::set(&public_key, &"rpId".into(), &JsValue::from_str(rp_id))
            .map_err(|_| "Failed to set rpId")?;
    }

    // Set userVerification
    if let Some(user_verification) = pk_json.get("userVerification").and_then(|u| u.as_str()) {
        Reflect::set(
            &public_key,
            &"userVerification".into(),
            &JsValue::from_str(user_verification),
        )
        .map_err(|_| "Failed to set userVerification")?;
    }

    // Set allowCredentials
    if let Some(allow) = pk_json.get("allowCredentials").and_then(|a| a.as_array()) {
        let allow_array = js_sys::Array::new();
        for cred in allow {
            let cred_obj = Object::new();
            if let Some(id) = cred.get("id").and_then(|i| i.as_str()) {
                let id_array = base64url_to_array(id)
                    .map_err(|e| format!("Failed to decode allow credential id: {:?}", e))?;
                Reflect::set(&cred_obj, &"id".into(), &id_array)
                    .map_err(|_| "Failed to set allow id")?;
            }
            if let Some(type_str) = cred.get("type").and_then(|t| t.as_str()) {
                Reflect::set(&cred_obj, &"type".into(), &JsValue::from_str(type_str))
                    .map_err(|_| "Failed to set allow type")?;
            }
            if let Some(transports) = cred.get("transports").and_then(|t| t.as_array()) {
                let transports_array = js_sys::Array::new();
                for transport in transports {
                    if let Some(t) = transport.as_str() {
                        transports_array.push(&JsValue::from_str(t));
                    }
                }
                Reflect::set(&cred_obj, &"transports".into(), &transports_array)
                    .map_err(|_| "Failed to set transports")?;
            }
            allow_array.push(&cred_obj);
        }
        Reflect::set(&public_key, &"allowCredentials".into(), &allow_array)
            .map_err(|_| "Failed to set allowCredentials")?;
    }

    Reflect::set(&options, &"publicKey".into(), &public_key)
        .map_err(|_| "Failed to set publicKey")?;

    Ok(options.into())
}

/// Serialize attestation response (from registration) for sending to backend
fn serialize_attestation_response(
    credential: &PublicKeyCredential,
) -> Result<serde_json::Value, String> {
    let response = credential.response();
    let attestation_response: web_sys::AuthenticatorAttestationResponse = response
        .dyn_into()
        .map_err(|_| "Failed to cast to AuthenticatorAttestationResponse".to_string())?;

    // Get raw ID
    let raw_id = Uint8Array::new(&credential.raw_id());
    let raw_id_b64 = array_to_base64url(&raw_id);

    // Get client data JSON
    let client_data_json = Uint8Array::new(&attestation_response.client_data_json());
    let client_data_b64 = array_to_base64url(&client_data_json);

    // Get attestation object
    let attestation_object = Uint8Array::new(&attestation_response.attestation_object());
    let attestation_object_b64 = array_to_base64url(&attestation_object);

    Ok(serde_json::json!({
        "id": credential.id(),
        "rawId": raw_id_b64,
        "type": credential.type_(),
        "response": {
            "clientDataJSON": client_data_b64,
            "attestationObject": attestation_object_b64
        }
    }))
}

/// Serialize assertion response (from authentication) for sending to backend
fn serialize_assertion_response(
    credential: &PublicKeyCredential,
) -> Result<serde_json::Value, String> {
    let response = credential.response();
    let assertion_response: web_sys::AuthenticatorAssertionResponse = response
        .dyn_into()
        .map_err(|_| "Failed to cast to AuthenticatorAssertionResponse".to_string())?;

    // Get raw ID
    let raw_id = Uint8Array::new(&credential.raw_id());
    let raw_id_b64 = array_to_base64url(&raw_id);

    // Get client data JSON
    let client_data_json = Uint8Array::new(&assertion_response.client_data_json());
    let client_data_b64 = array_to_base64url(&client_data_json);

    // Get authenticator data
    let authenticator_data = Uint8Array::new(&assertion_response.authenticator_data());
    let authenticator_data_b64 = array_to_base64url(&authenticator_data);

    // Get signature
    let signature = Uint8Array::new(&assertion_response.signature());
    let signature_b64 = array_to_base64url(&signature);

    // Get user handle (optional)
    let user_handle = assertion_response.user_handle().map(|uh| {
        let uh_array = Uint8Array::new(&uh);
        array_to_base64url(&uh_array)
    });

    let mut response_obj = serde_json::json!({
        "clientDataJSON": client_data_b64,
        "authenticatorData": authenticator_data_b64,
        "signature": signature_b64
    });

    if let Some(uh) = user_handle {
        response_obj["userHandle"] = serde_json::Value::String(uh);
    }

    Ok(serde_json::json!({
        "id": credential.id(),
        "rawId": raw_id_b64,
        "type": credential.type_(),
        "response": response_obj
    }))
}
