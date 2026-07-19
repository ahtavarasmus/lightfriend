use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

pub const DEVICE_TOKEN_PREFIX: &str = "lft_";
const DEVICE_TOKEN_RANDOM_BYTES: usize = 32;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LightToolIdentityError {
    #[error("installation id must be a UUID")]
    InvalidInstallationId,
    #[error("device token has an invalid format")]
    InvalidDeviceToken,
}

pub struct GeneratedDeviceToken {
    pub raw: String,
    pub hash: String,
}

pub fn hash_installation_id(installation_id: &str) -> Result<String, LightToolIdentityError> {
    let canonical = Uuid::parse_str(installation_id)
        .map_err(|_| LightToolIdentityError::InvalidInstallationId)?
        .to_string();
    Ok(sha256_hex(canonical.as_bytes()))
}

pub fn generate_device_token() -> GeneratedDeviceToken {
    let mut random_bytes = [0_u8; DEVICE_TOKEN_RANDOM_BYTES];
    OsRng.fill_bytes(&mut random_bytes);
    let raw = format!("{DEVICE_TOKEN_PREFIX}{}", hex::encode(random_bytes));
    let hash = sha256_hex(raw.as_bytes());
    GeneratedDeviceToken { raw, hash }
}

pub fn hash_device_token(device_token: &str) -> Result<String, LightToolIdentityError> {
    if !has_valid_device_token_format(device_token) {
        return Err(LightToolIdentityError::InvalidDeviceToken);
    }
    Ok(sha256_hex(device_token.as_bytes()))
}

fn has_valid_device_token_format(device_token: &str) -> bool {
    let Some(random_hex) = device_token.strip_prefix(DEVICE_TOKEN_PREFIX) else {
        return false;
    };
    random_hex.len() == DEVICE_TOKEN_RANDOM_BYTES * 2
        && random_hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256_hex(value: &[u8]) -> String {
    hex::encode(Sha256::digest(value))
}
