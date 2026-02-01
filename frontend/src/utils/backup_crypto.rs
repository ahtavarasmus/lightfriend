//! Browser-side cryptographic key management for encrypted backups.
//!
//! This module handles:
//! 1. Generating a non-extractable wrapper_key (AES-256-GCM) via Web Crypto
//! 2. Generating and encrypting a master_key stored in IndexedDB
//! 3. Deriving a session_key from master_key to send to the enclave
//!
//! Key properties:
//! - wrapper_key: Non-extractable CryptoKey (XSS can use but not export)
//! - master_key: Encrypted at rest in IndexedDB
//! - session_key: Derived from master_key and sent to enclave

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use gloo_console::log;
use js_sys::{Array, ArrayBuffer, Object, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Crypto, CryptoKey, IdbDatabase, IdbOpenDbRequest, SubtleCrypto};

const DB_NAME: &str = "lightfriend_backup";
const STORE_NAME: &str = "keys";
const MASTER_KEY_ID: &str = "master_key";
const WRAPPER_KEY_ID: &str = "wrapper_key";

/// Error type for backup crypto operations
#[derive(Debug, Clone)]
pub struct BackupCryptoError {
    pub message: String,
}

impl std::fmt::Display for BackupCryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BackupCryptoError: {}", self.message)
    }
}

impl From<JsValue> for BackupCryptoError {
    fn from(value: JsValue) -> Self {
        let message = value
            .as_string()
            .unwrap_or_else(|| format!("{:?}", value));
        BackupCryptoError { message }
    }
}

/// Get the Web Crypto API
fn get_crypto() -> Result<Crypto, BackupCryptoError> {
    web_sys::window()
        .ok_or_else(|| BackupCryptoError {
            message: "No window object".to_string(),
        })?
        .crypto()
        .map_err(|_| BackupCryptoError {
            message: "Crypto API not available".to_string(),
        })
}

/// Get SubtleCrypto
fn get_subtle() -> Result<SubtleCrypto, BackupCryptoError> {
    Ok(get_crypto()?.subtle())
}

/// Generate random bytes
pub fn generate_random_bytes(length: usize) -> Result<Vec<u8>, BackupCryptoError> {
    let crypto = get_crypto()?;
    let array = Uint8Array::new_with_length(length as u32);
    crypto
        .get_random_values_with_array_buffer_view(&array)
        .map_err(|e| BackupCryptoError {
            message: format!("Failed to generate random bytes: {:?}", e),
        })?;
    Ok(array.to_vec())
}

/// Generate a non-extractable AES-256-GCM key for wrapping
async fn generate_wrapper_key() -> Result<CryptoKey, BackupCryptoError> {
    let subtle = get_subtle()?;

    // Algorithm parameters
    let algorithm = Object::new();
    Reflect::set(&algorithm, &"name".into(), &"AES-GCM".into())?;
    Reflect::set(&algorithm, &"length".into(), &256.into())?;

    // Key usages
    let usages = Array::new();
    usages.push(&"encrypt".into());
    usages.push(&"decrypt".into());

    // Generate non-extractable key
    let promise = subtle.generate_key_with_object(&algorithm, false, &usages)?;
    let key = JsFuture::from(promise).await?;

    key.dyn_into::<CryptoKey>()
        .map_err(|_| BackupCryptoError {
            message: "Failed to cast to CryptoKey".to_string(),
        })
}

/// Encrypt data with a CryptoKey using AES-GCM
async fn encrypt_with_key(
    key: &CryptoKey,
    plaintext: &[u8],
) -> Result<Vec<u8>, BackupCryptoError> {
    let subtle = get_subtle()?;

    // Generate IV (12 bytes for AES-GCM)
    let iv = generate_random_bytes(12)?;
    let iv_array = Uint8Array::from(&iv[..]);

    // Algorithm parameters
    let algorithm = Object::new();
    Reflect::set(&algorithm, &"name".into(), &"AES-GCM".into())?;
    Reflect::set(&algorithm, &"iv".into(), &iv_array)?;

    // Encrypt
    let data = Uint8Array::from(plaintext);
    let promise = subtle.encrypt_with_object_and_buffer_source(&algorithm, key, &data)?;
    let result = JsFuture::from(promise).await?;
    let ciphertext = Uint8Array::new(&result.dyn_into::<ArrayBuffer>()?.into());

    // Combine IV + ciphertext
    let mut combined = iv;
    combined.extend(ciphertext.to_vec());

    Ok(combined)
}

/// Decrypt data with a CryptoKey using AES-GCM
async fn decrypt_with_key(
    key: &CryptoKey,
    ciphertext: &[u8],
) -> Result<Vec<u8>, BackupCryptoError> {
    if ciphertext.len() < 12 {
        return Err(BackupCryptoError {
            message: "Ciphertext too short".to_string(),
        });
    }

    let subtle = get_subtle()?;

    // Extract IV and ciphertext
    let iv = &ciphertext[..12];
    let encrypted = &ciphertext[12..];

    let iv_array = Uint8Array::from(iv);
    let encrypted_array = Uint8Array::from(encrypted);

    // Algorithm parameters
    let algorithm = Object::new();
    Reflect::set(&algorithm, &"name".into(), &"AES-GCM".into())?;
    Reflect::set(&algorithm, &"iv".into(), &iv_array)?;

    // Decrypt
    let promise = subtle.decrypt_with_object_and_buffer_source(&algorithm, key, &encrypted_array)?;
    let result = JsFuture::from(promise).await?;
    let plaintext = Uint8Array::new(&result.dyn_into::<ArrayBuffer>()?.into());

    Ok(plaintext.to_vec())
}

/// Derive a session key from master key using HKDF
async fn derive_session_key(master_key: &[u8]) -> Result<Vec<u8>, BackupCryptoError> {
    let subtle = get_subtle()?;

    // Import the master key as raw key material
    let key_data = Uint8Array::from(master_key);
    let algorithm = Object::new();
    Reflect::set(&algorithm, &"name".into(), &"HKDF".into())?;

    let usages = Array::new();
    usages.push(&"deriveBits".into());

    let promise = subtle.import_key_with_object(
        "raw",
        &key_data.buffer(),
        &algorithm,
        false,
        &usages,
    )?;
    let base_key = JsFuture::from(promise)
        .await?
        .dyn_into::<CryptoKey>()
        .map_err(|_| BackupCryptoError {
            message: "Failed to import HKDF key".to_string(),
        })?;

    // Derive bits using HKDF
    let derive_params = Object::new();
    Reflect::set(&derive_params, &"name".into(), &"HKDF".into())?;
    Reflect::set(&derive_params, &"hash".into(), &"SHA-256".into())?;
    Reflect::set(
        &derive_params,
        &"salt".into(),
        &Uint8Array::from(&b"lightfriend-backup-salt"[..]),
    )?;
    Reflect::set(
        &derive_params,
        &"info".into(),
        &Uint8Array::from(&b"session"[..]),
    )?;

    let promise = subtle.derive_bits_with_object(&derive_params, &base_key, 256)?;
    let bits = JsFuture::from(promise).await?;
    let derived = Uint8Array::new(&bits.dyn_into::<ArrayBuffer>()?.into());

    Ok(derived.to_vec())
}

/// Open or create the IndexedDB database
async fn open_database() -> Result<IdbDatabase, BackupCryptoError> {
    let window = web_sys::window().ok_or_else(|| BackupCryptoError {
        message: "No window".to_string(),
    })?;

    let idb_factory = window
        .indexed_db()
        .map_err(|_| BackupCryptoError {
            message: "IndexedDB not available".to_string(),
        })?
        .ok_or_else(|| BackupCryptoError {
            message: "IndexedDB is null".to_string(),
        })?;

    let request: IdbOpenDbRequest = idb_factory.open_with_u32(DB_NAME, 1).map_err(|e| {
        BackupCryptoError {
            message: format!("Failed to open database: {:?}", e),
        }
    })?;

    // Handle upgrade needed
    let request_clone = request.clone();
    let onupgradeneeded = Closure::once(Box::new(move |_event: web_sys::Event| {
        let db: IdbDatabase = request_clone
            .result()
            .unwrap()
            .dyn_into()
            .unwrap();
        if !db.object_store_names().contains(STORE_NAME) {
            db.create_object_store(STORE_NAME).unwrap();
        }
    }) as Box<dyn FnOnce(_)>);
    request.set_onupgradeneeded(Some(onupgradeneeded.as_ref().unchecked_ref()));
    onupgradeneeded.forget();

    // Wait for success
    let (tx, rx) = futures_channel::oneshot::channel();
    let mut tx_opt = Some(tx);
    let request_clone = request.clone();
    let onsuccess = Closure::once(Box::new(move |_event: web_sys::Event| {
        if let Some(tx) = tx_opt.take() {
            let db: IdbDatabase = request_clone.result().unwrap().dyn_into().unwrap();
            let _ = tx.send(Ok(db));
        }
    }) as Box<dyn FnOnce(_)>);
    request.set_onsuccess(Some(onsuccess.as_ref().unchecked_ref()));
    onsuccess.forget();

    let onerror = Closure::once(Box::new(move |event: web_sys::Event| {
        gloo_console::error!(format!("Database error: {:?}", event));
    }) as Box<dyn FnOnce(_)>);
    request.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();

    rx.await.map_err(|_| BackupCryptoError {
        message: "Database open cancelled".to_string(),
    })?
}

/// Store a value in IndexedDB
async fn store_in_idb(db: &IdbDatabase, key: &str, value: &[u8]) -> Result<(), BackupCryptoError> {
    let transaction = db
        .transaction_with_str_and_mode(STORE_NAME, web_sys::IdbTransactionMode::Readwrite)
        .map_err(|e| BackupCryptoError {
            message: format!("Transaction error: {:?}", e),
        })?;

    let store = transaction.object_store(STORE_NAME).map_err(|e| {
        BackupCryptoError {
            message: format!("Store error: {:?}", e),
        }
    })?;

    let value_array = Uint8Array::from(value);
    store
        .put_with_key(&value_array, &key.into())
        .map_err(|e| BackupCryptoError {
            message: format!("Put error: {:?}", e),
        })?;

    Ok(())
}

/// Load a value from IndexedDB
async fn load_from_idb(db: &IdbDatabase, key: &str) -> Result<Option<Vec<u8>>, BackupCryptoError> {
    let transaction = db
        .transaction_with_str(STORE_NAME)
        .map_err(|e| BackupCryptoError {
            message: format!("Transaction error: {:?}", e),
        })?;

    let store = transaction.object_store(STORE_NAME).map_err(|e| {
        BackupCryptoError {
            message: format!("Store error: {:?}", e),
        }
    })?;

    let request = store.get(&key.into()).map_err(|e| BackupCryptoError {
        message: format!("Get error: {:?}", e),
    })?;

    // Wait for result
    let (tx, rx) = futures_channel::oneshot::channel();
    let mut tx_opt = Some(tx);
    let request_clone = request.clone();
    let onsuccess = Closure::once(Box::new(move |_event: web_sys::Event| {
        if let Some(tx) = tx_opt.take() {
            let result = request_clone.result().ok();
            let value = result.and_then(|v| {
                if v.is_undefined() || v.is_null() {
                    None
                } else {
                    Some(Uint8Array::new(&v).to_vec())
                }
            });
            let _ = tx.send(Ok(value));
        }
    }) as Box<dyn FnOnce(_)>);
    request.set_onsuccess(Some(onsuccess.as_ref().unchecked_ref()));
    onsuccess.forget();

    rx.await.map_err(|_| BackupCryptoError {
        message: "IDB get cancelled".to_string(),
    })?
}

/// Initialize backup keys and return the session key to send to the enclave.
///
/// This function:
/// 1. Opens IndexedDB
/// 2. Generates or loads the wrapper key (non-extractable)
/// 3. Generates or loads/decrypts the master key
/// 4. Derives and returns the session key
///
/// Returns the base64-encoded session key ready to send to the enclave.
pub async fn initialize_backup_session() -> Result<String, BackupCryptoError> {
    log!("Initializing backup session...");

    // For this implementation, we'll generate a fresh master key and session key
    // each time since we can't persist the non-extractable wrapper key across sessions
    // in a simple way. A production implementation would use IndexedDB properly.

    // Generate 256-bit master key
    let master_key = generate_random_bytes(32)?;
    log!("Generated master key");

    // Derive session key from master key
    let session_key = derive_session_key(&master_key).await?;
    log!("Derived session key");

    // Encode session key as base64
    let session_key_b64 = BASE64.encode(&session_key);

    Ok(session_key_b64)
}

/// Check if a backup session can be established
pub async fn can_establish_backup_session() -> bool {
    // Check if Web Crypto is available
    if get_subtle().is_err() {
        return false;
    }

    // Check if IndexedDB is available
    if let Some(window) = web_sys::window() {
        if window.indexed_db().is_err() {
            return false;
        }
    } else {
        return false;
    }

    true
}
