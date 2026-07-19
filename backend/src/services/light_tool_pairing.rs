use crate::{
    models::light_tool_models::NewLightToolPairingSession,
    repositories::light_tool_pairing_repository::{
        LightToolPairingRepository, LightToolPairingRepositoryError, PairingConsumption,
    },
};
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;

pub const PAIRING_TTL_SECONDS: i32 = 5 * 60;
const PAIRING_SCHEME: &str = "lightfriend";
const PAIRING_HOST: &str = "pair";
const PAIRING_TOKEN_PREFIX: &str = "lfp_";
const PAIRING_TOKEN_RANDOM_BYTES: usize = 32;

pub struct LightToolPairingOffer {
    pub pairing_uri: String,
    pub expires_at: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightToolPairingStatus {
    None,
    Pending,
    Connected,
    Expired,
}

#[derive(Debug, Error)]
pub enum LightToolPairingError {
    #[error("pairing code is invalid or expired")]
    InvalidOrExpired,
    #[error("device is unavailable")]
    DeviceUnavailable,
    #[error("device is already connected to an account")]
    DeviceAlreadyLinked,
    #[error(transparent)]
    Repository(#[from] LightToolPairingRepositoryError),
}

pub struct LightToolPairingService {
    repository: LightToolPairingRepository,
}

impl LightToolPairingService {
    pub fn new(repository: LightToolPairingRepository) -> Self {
        Self { repository }
    }

    pub fn create_offer(
        &self,
        user_id: i32,
        now: i32,
    ) -> Result<LightToolPairingOffer, LightToolPairingError> {
        let raw_token = generate_pairing_token();
        let expires_at = now + PAIRING_TTL_SECONDS;
        self.repository
            .upsert_for_user(&NewLightToolPairingSession {
                user_id,
                token_hash: hash_pairing_token(&raw_token),
                expires_at,
                created_at: now,
            })?;

        Ok(LightToolPairingOffer {
            pairing_uri: format!("{PAIRING_SCHEME}://{PAIRING_HOST}?token={raw_token}"),
            expires_at,
        })
    }

    pub fn consume_uri(
        &self,
        pairing_uri: &str,
        device_id: i32,
        now: i32,
    ) -> Result<PairingConsumption, LightToolPairingError> {
        let token = pairing_token_from_uri(pairing_uri)?;
        self.repository
            .consume(&hash_pairing_token(&token), device_id, now)
            .map_err(Into::into)
    }

    pub fn status_for_user(
        &self,
        user_id: i32,
        now: i32,
    ) -> Result<LightToolPairingStatus, LightToolPairingError> {
        let Some(session) = self.repository.find_for_user(user_id)? else {
            return Ok(LightToolPairingStatus::None);
        };
        if session.consumed_at.is_some() {
            return Ok(LightToolPairingStatus::Connected);
        }
        if session.expires_at <= now {
            return Ok(LightToolPairingStatus::Expired);
        }
        Ok(LightToolPairingStatus::Pending)
    }
}

fn generate_pairing_token() -> String {
    let mut random_bytes = [0_u8; PAIRING_TOKEN_RANDOM_BYTES];
    OsRng.fill_bytes(&mut random_bytes);
    format!("{PAIRING_TOKEN_PREFIX}{}", hex::encode(random_bytes))
}

fn pairing_token_from_uri(pairing_uri: &str) -> Result<String, LightToolPairingError> {
    let parsed = Url::parse(pairing_uri).map_err(|_| LightToolPairingError::InvalidOrExpired)?;
    if parsed.scheme() != PAIRING_SCHEME
        || parsed.host_str() != Some(PAIRING_HOST)
        || parsed.path() != ""
        || parsed.fragment().is_some()
    {
        return Err(LightToolPairingError::InvalidOrExpired);
    }
    let pairs = parsed.query_pairs().collect::<Vec<_>>();
    if pairs.len() != 1 || pairs[0].0 != "token" || !has_valid_pairing_token(&pairs[0].1) {
        return Err(LightToolPairingError::InvalidOrExpired);
    }
    Ok(pairs[0].1.to_string())
}

fn has_valid_pairing_token(token: &str) -> bool {
    let Some(random_hex) = token.strip_prefix(PAIRING_TOKEN_PREFIX) else {
        return false;
    };
    random_hex.len() == PAIRING_TOKEN_RANDOM_BYTES * 2
        && random_hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn hash_pairing_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}
