//! Browser-side cryptographic key management for encrypted backups.
//!
//! This module handles:
//! 1. Deriving a master_key from user password via PBKDF2
//! 2. Generating a non-extractable wrapper_key (AES-256-GCM) via Web Crypto
//! 3. Wrapping the master_key with the wrapper_key for IndexedDB storage
//! 4. Deriving a session_key from master_key to send to the enclave
//!
//! Key properties:
//! - wrapper_key: Non-extractable CryptoKey (XSS can use but not export)
//! - master_key: Derived from password, wrapped at rest in IndexedDB
//! - session_key: Derived from master_key and sent to enclave
//!
//! Security: Same password = same master_key on all devices

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use gloo_console::log;
use js_sys::{Array, ArrayBuffer, Object, Reflect, Uint8Array};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Crypto, CryptoKey, IdbDatabase, IdbOpenDbRequest, SubtleCrypto};

const DB_NAME: &str = "lightfriend_backup";
const STORE_NAME: &str = "keys";
const WRAPPED_MASTER_KEY_ID: &str = "wrapped_master_key";
const WRAPPER_KEY_ID: &str = "wrapper_key";
const PBKDF2_ITERATIONS: u32 = 100_000;

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

/// Store a CryptoKey directly in IndexedDB (preserves non-extractable property)
async fn store_crypto_key_in_idb(
    db: &IdbDatabase,
    key: &str,
    crypto_key: &CryptoKey,
) -> Result<(), BackupCryptoError> {
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

    // Store the CryptoKey directly - IndexedDB's structured clone will preserve it
    store
        .put_with_key(crypto_key, &key.into())
        .map_err(|e| BackupCryptoError {
            message: format!("Put CryptoKey error: {:?}", e),
        })?;

    Ok(())
}

/// Load a CryptoKey from IndexedDB
async fn load_crypto_key_from_idb(
    db: &IdbDatabase,
    key: &str,
) -> Result<Option<CryptoKey>, BackupCryptoError> {
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
                    v.dyn_into::<CryptoKey>().ok()
                }
            });
            let _ = tx.send(Ok(value));
        }
    }) as Box<dyn FnOnce(_)>);
    request.set_onsuccess(Some(onsuccess.as_ref().unchecked_ref()));
    onsuccess.forget();

    rx.await.map_err(|_| BackupCryptoError {
        message: "IDB get CryptoKey cancelled".to_string(),
    })?
}

/// Derive master key from password using PBKDF2-HMAC-SHA256
///
/// Uses a deterministic salt based on user_id so the same password
/// always produces the same master key across devices.
pub fn derive_master_key_from_password(
    password: &str,
    user_id: i32,
) -> Result<[u8; 32], BackupCryptoError> {
    // Deterministic salt: prefix + user_id
    let mut salt = b"lightfriend-backup-v1".to_vec();
    salt.extend_from_slice(&user_id.to_le_bytes());

    let mut master_key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, PBKDF2_ITERATIONS, &mut master_key);

    Ok(master_key)
}

/// Wrap a master key using Web Crypto API with AES-GCM
/// Returns IV (12 bytes) + wrapped key
async fn wrap_master_key(
    wrapper_key: &CryptoKey,
    master_key: &[u8; 32],
) -> Result<Vec<u8>, BackupCryptoError> {
    let subtle = get_subtle()?;

    // Generate IV for wrapping
    let iv = generate_random_bytes(12)?;
    let iv_array = Uint8Array::from(&iv[..]);

    // First import the master key as a raw key for wrapping
    let key_data = Uint8Array::from(&master_key[..]);
    let import_algorithm = Object::new();
    Reflect::set(&import_algorithm, &"name".into(), &"AES-GCM".into())?;
    Reflect::set(&import_algorithm, &"length".into(), &256.into())?;

    let usages = Array::new();
    usages.push(&"encrypt".into());
    usages.push(&"decrypt".into());

    // Import as extractable so we can wrap it
    let promise = subtle.import_key_with_object(
        "raw",
        &key_data.buffer(),
        &import_algorithm,
        true, // extractable for wrapping
        &usages,
    )?;
    let key_to_wrap = JsFuture::from(promise)
        .await?
        .dyn_into::<CryptoKey>()
        .map_err(|_| BackupCryptoError {
            message: "Failed to import key for wrapping".to_string(),
        })?;

    // Wrap using AES-GCM
    let wrap_algorithm = Object::new();
    Reflect::set(&wrap_algorithm, &"name".into(), &"AES-GCM".into())?;
    Reflect::set(&wrap_algorithm, &"iv".into(), &iv_array)?;

    let promise = subtle.wrap_key_with_object("raw", &key_to_wrap, wrapper_key, &wrap_algorithm)?;
    let wrapped = JsFuture::from(promise).await?;
    let wrapped_bytes = Uint8Array::new(&wrapped.dyn_into::<ArrayBuffer>()?.into());

    // Combine IV + wrapped key
    let mut result = iv;
    result.extend(wrapped_bytes.to_vec());

    Ok(result)
}

/// Unwrap a master key as HKDF key material (non-extractable)
/// Input: IV (12 bytes) + wrapped key
/// Returns a CryptoKey that can only be used for deriveBits - cannot be exported
async fn unwrap_master_key_for_hkdf(
    wrapper_key: &CryptoKey,
    wrapped_data: &[u8],
) -> Result<CryptoKey, BackupCryptoError> {
    if wrapped_data.len() < 12 {
        return Err(BackupCryptoError {
            message: "Wrapped data too short".to_string(),
        });
    }

    let subtle = get_subtle()?;

    // Extract IV and wrapped key
    let iv = &wrapped_data[..12];
    let wrapped_key = &wrapped_data[12..];

    let iv_array = Uint8Array::from(iv);
    let wrapped_array = Uint8Array::from(wrapped_key);

    // Unwrap algorithm (AES-GCM decryption)
    let unwrap_algorithm = Object::new();
    Reflect::set(&unwrap_algorithm, &"name".into(), &"AES-GCM".into())?;
    Reflect::set(&unwrap_algorithm, &"iv".into(), &iv_array)?;

    // Key algorithm: HKDF for derivation
    let key_algorithm = Object::new();
    Reflect::set(&key_algorithm, &"name".into(), &"HKDF".into())?;

    // Usages: only deriveBits
    let usages = Array::new();
    usages.push(&"deriveBits".into());

    // Non-extractable: XSS cannot export the key material
    let promise = subtle.unwrap_key_with_buffer_source_and_object_and_object(
        "raw",
        &wrapped_array.into(),
        wrapper_key,
        &unwrap_algorithm,
        &key_algorithm,
        false, // non-extractable - this is the security benefit
        &usages,
    )?;
    let unwrapped = JsFuture::from(promise).await?;

    unwrapped.dyn_into::<CryptoKey>().map_err(|_| BackupCryptoError {
        message: "Failed to unwrap master key as HKDF".to_string(),
    })
}

/// Derive session key directly from an HKDF CryptoKey (non-extractable)
async fn derive_session_key_from_crypto_key(
    hkdf_key: &CryptoKey,
) -> Result<Vec<u8>, BackupCryptoError> {
    let subtle = get_subtle()?;

    // HKDF derivation parameters
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

    let promise = subtle.derive_bits_with_object(&derive_params, hkdf_key, 256)?;
    let bits = JsFuture::from(promise).await?;
    let derived = Uint8Array::new(&bits.dyn_into::<ArrayBuffer>()?.into());

    Ok(derived.to_vec())
}

/// Initialize backup with password - called after login
///
/// This function:
/// 1. Derives master key from password using PBKDF2
/// 2. Gets or creates a non-extractable wrapper key
/// 3. Wraps the master key and stores in IndexedDB
/// 4. Derives and returns the session key
///
/// Returns the base64-encoded session key ready to send to the enclave.
pub async fn initialize_backup_with_password(
    password: &str,
    user_id: i32,
) -> Result<String, BackupCryptoError> {
    log!("Initializing backup with password...");

    // Step 1: Derive master key from password
    let master_key = derive_master_key_from_password(password, user_id)?;
    log!("Derived master key from password");

    // Step 2: Open IndexedDB
    let db = open_database().await?;

    // Step 3: Get or create wrapper key
    let wrapper_key = match load_crypto_key_from_idb(&db, WRAPPER_KEY_ID).await? {
        Some(key) => {
            log!("Loaded existing wrapper key");
            key
        }
        None => {
            log!("Generating new wrapper key");
            let key = generate_wrapper_key().await?;
            store_crypto_key_in_idb(&db, WRAPPER_KEY_ID, &key).await?;
            key
        }
    };

    // Step 4: Wrap and store the master key
    let wrapped_master_key = wrap_master_key(&wrapper_key, &master_key).await?;
    store_in_idb(&db, WRAPPED_MASTER_KEY_ID, &wrapped_master_key).await?;
    log!("Stored wrapped master key");

    // Step 5: Derive session key from master key
    let session_key = derive_session_key(&master_key).await?;
    log!("Derived session key");

    // Encode session key as base64
    let session_key_b64 = BASE64.encode(&session_key);

    Ok(session_key_b64)
}

/// Get session key from storage (for periodic refresh)
///
/// This function:
/// 1. Loads the wrapper key from IndexedDB
/// 2. Loads and unwraps the master key as non-extractable HKDF key
/// 3. Derives session key directly (no export needed)
///
/// Security: The master key is never extractable - XSS can derive session keys
/// but cannot steal the master key material itself.
pub async fn get_session_key_from_storage() -> Result<Option<String>, BackupCryptoError> {
    // Open IndexedDB
    let db = open_database().await?;

    // Load wrapper key
    let wrapper_key = match load_crypto_key_from_idb(&db, WRAPPER_KEY_ID).await? {
        Some(key) => key,
        None => return Ok(None), // No backup initialized yet
    };

    // Load wrapped master key
    let wrapped_master_key = match load_from_idb(&db, WRAPPED_MASTER_KEY_ID).await? {
        Some(data) => data,
        None => return Ok(None), // No backup initialized yet
    };

    // Unwrap master key as non-extractable HKDF key
    let hkdf_key = unwrap_master_key_for_hkdf(&wrapper_key, &wrapped_master_key).await?;

    // Derive session key directly from the CryptoKey (no export possible)
    let session_key = derive_session_key_from_crypto_key(&hkdf_key).await?;

    // Encode session key as base64
    let session_key_b64 = BASE64.encode(&session_key);

    Ok(Some(session_key_b64))
}
