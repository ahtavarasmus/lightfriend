use axum::http::HeaderMap;
use governor::{DefaultKeyedRateLimiter, Quota, RateLimiter};
use sha2::{Digest, Sha256};
use std::num::NonZeroU32;
use std::time::Duration;

type Limiter = DefaultKeyedRateLimiter<[u8; 32]>;

#[derive(Clone, Copy, Debug)]
pub enum RateLimitScope {
    Api,
    Login,
    PhoneVerifyRequest,
    PhoneVerifyAttempt,
    TotpVerify,
    WebauthnVerify,
    AgentPairingStart,
    AgentPairingPoll,
}

pub struct SecurityRateLimiters {
    api: Limiter,
    login: Limiter,
    phone_verify_request: Limiter,
    phone_verify_attempt: Limiter,
    totp_verify: Limiter,
    webauthn_verify: Limiter,
    agent_pairing_start: Limiter,
    agent_pairing_poll: Limiter,
}

impl Default for SecurityRateLimiters {
    fn default() -> Self {
        Self {
            api: RateLimiter::keyed(
                Quota::per_minute(NonZeroU32::new(120).unwrap())
                    .allow_burst(NonZeroU32::new(30).unwrap()),
            ),
            login: RateLimiter::keyed(Quota::per_minute(NonZeroU32::new(5).unwrap())),
            phone_verify_request: RateLimiter::keyed(Quota::per_hour(NonZeroU32::new(3).unwrap())),
            phone_verify_attempt: RateLimiter::keyed(
                Quota::with_period(Duration::from_secs(60 * 60))
                    .unwrap()
                    .allow_burst(NonZeroU32::new(3).unwrap()),
            ),
            totp_verify: RateLimiter::keyed(
                Quota::with_period(Duration::from_secs(3 * 60))
                    .unwrap()
                    .allow_burst(NonZeroU32::new(5).unwrap()),
            ),
            webauthn_verify: RateLimiter::keyed(Quota::per_minute(NonZeroU32::new(5).unwrap())),
            agent_pairing_start: RateLimiter::keyed(
                Quota::per_minute(NonZeroU32::new(6).unwrap())
                    .allow_burst(NonZeroU32::new(6).unwrap()),
            ),
            agent_pairing_poll: RateLimiter::keyed(
                Quota::per_minute(NonZeroU32::new(30).unwrap())
                    .allow_burst(NonZeroU32::new(6).unwrap()),
            ),
        }
    }
}

impl SecurityRateLimiters {
    pub fn check(&self, scope: RateLimitScope, raw_key: &str) -> bool {
        let normalized = normalize_key(scope, raw_key);
        let key: [u8; 32] = Sha256::digest(normalized.as_bytes()).into();
        self.limiter(scope).check_key(&key).is_ok()
    }

    pub fn retain_recent(&self) {
        for limiter in self.all() {
            limiter.retain_recent();
            limiter.shrink_to_fit();
        }
    }

    fn limiter(&self, scope: RateLimitScope) -> &Limiter {
        match scope {
            RateLimitScope::Api => &self.api,
            RateLimitScope::Login => &self.login,
            RateLimitScope::PhoneVerifyRequest => &self.phone_verify_request,
            RateLimitScope::PhoneVerifyAttempt => &self.phone_verify_attempt,
            RateLimitScope::TotpVerify => &self.totp_verify,
            RateLimitScope::WebauthnVerify => &self.webauthn_verify,
            RateLimitScope::AgentPairingStart => &self.agent_pairing_start,
            RateLimitScope::AgentPairingPoll => &self.agent_pairing_poll,
        }
    }

    fn all(&self) -> [&Limiter; 8] {
        [
            &self.api,
            &self.login,
            &self.phone_verify_request,
            &self.phone_verify_attempt,
            &self.totp_verify,
            &self.webauthn_verify,
            &self.agent_pairing_start,
            &self.agent_pairing_poll,
        ]
    }
}

pub fn client_identity(headers: &HeaderMap) -> &str {
    headers
        .get("cf-connecting-ip")
        .or_else(|| headers.get("x-real-ip"))
        .or_else(|| headers.get("x-forwarded-for"))
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(',').next().unwrap_or(value).trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown-client")
}

fn normalize_key(scope: RateLimitScope, raw_key: &str) -> String {
    match scope {
        RateLimitScope::Login => raw_key.trim().to_lowercase(),
        RateLimitScope::PhoneVerifyRequest | RateLimitScope::PhoneVerifyAttempt => raw_key
            .chars()
            .filter(|character| character.is_ascii_digit() || *character == '+')
            .collect(),
        _ => raw_key.trim().to_string(),
    }
}
