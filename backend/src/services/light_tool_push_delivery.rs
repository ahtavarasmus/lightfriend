use crate::{
    repositories::light_tool_push_repository::{
        LightToolPushRepository, LightToolPushRepositoryError,
    },
    PgDbPool,
};
use reqwest::{redirect::Policy, StatusCode};
use std::{collections::HashSet, env, time::Duration};
use thiserror::Error;
use url::Url;

pub const CONVERSATION_CHANGED_PAYLOAD: &[u8] = b"conversation_changed";
const ALLOWED_HOSTS_ENV: &str = "LIGHT_TOOL_PUSH_ALLOWED_HOSTS";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightToolPushDeliveryOutcome {
    NoEndpoint,
    Delivered,
    EndpointExpired,
}

#[derive(Debug, Error)]
pub enum LightToolPushDeliveryError {
    #[error(transparent)]
    Repository(#[from] LightToolPushRepositoryError),
    #[error("push endpoint URL is invalid")]
    InvalidEndpoint,
    #[error("push endpoint host is not allowed")]
    HostNotAllowed,
    #[error("could not create push HTTP client: {0}")]
    ClientBuild(reqwest::Error),
    #[error("push request failed: {0}")]
    Request(reqwest::Error),
    #[error("push endpoint returned HTTP {0}")]
    HttpStatus(StatusCode),
}

pub struct LightToolPushDeliveryService {
    repository: LightToolPushRepository,
    client: reqwest::Client,
    allowed_hosts: HashSet<String>,
}

impl LightToolPushDeliveryService {
    pub fn from_env(pool: PgDbPool) -> Result<Self, LightToolPushDeliveryError> {
        let mut allowed_hosts = env::var(ALLOWED_HOSTS_ENV)
            .unwrap_or_default()
            .split(',')
            .map(canonical_host)
            .filter(|host| !host.is_empty())
            .collect::<HashSet<_>>();

        if cfg!(debug_assertions) {
            allowed_hosts.extend(
                ["localhost", "127.0.0.1", "::1"]
                    .into_iter()
                    .map(str::to_string),
            );
        }

        Self::new(pool, allowed_hosts)
    }

    pub fn new<I, S>(pool: PgDbPool, allowed_hosts: I) -> Result<Self, LightToolPushDeliveryError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .redirect(Policy::none())
            .no_proxy()
            .build()
            .map_err(LightToolPushDeliveryError::ClientBuild)?;
        let allowed_hosts = allowed_hosts
            .into_iter()
            .map(|host| canonical_host(host.as_ref()))
            .filter(|host| !host.is_empty())
            .collect();

        Ok(Self {
            repository: LightToolPushRepository::new(pool),
            client,
            allowed_hosts,
        })
    }

    pub async fn send_conversation_changed(
        &self,
        device_id: i32,
    ) -> Result<LightToolPushDeliveryOutcome, LightToolPushDeliveryError> {
        let Some(registration) = self.repository.find_for_device(device_id)? else {
            return Ok(LightToolPushDeliveryOutcome::NoEndpoint);
        };
        self.validate_destination(&registration.endpoint)?;

        let response = self
            .client
            .post(&registration.endpoint)
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(CONVERSATION_CHANGED_PAYLOAD)
            .send()
            .await
            .map_err(LightToolPushDeliveryError::Request)?;
        let status = response.status();

        if status.is_success() {
            return Ok(LightToolPushDeliveryOutcome::Delivered);
        }
        if status == StatusCode::NOT_FOUND || status == StatusCode::GONE {
            self.repository
                .delete_for_device_if_endpoint_hash(device_id, &registration.endpoint_hash)?;
            return Ok(LightToolPushDeliveryOutcome::EndpointExpired);
        }

        Err(LightToolPushDeliveryError::HttpStatus(status))
    }

    fn validate_destination(&self, endpoint: &str) -> Result<(), LightToolPushDeliveryError> {
        let url = Url::parse(endpoint).map_err(|_| LightToolPushDeliveryError::InvalidEndpoint)?;
        let host = url
            .host_str()
            .map(canonical_host)
            .ok_or(LightToolPushDeliveryError::InvalidEndpoint)?;
        if !self.allowed_hosts.contains(&host) {
            return Err(LightToolPushDeliveryError::HostNotAllowed);
        }

        let valid_scheme = url.scheme() == "https"
            || (cfg!(debug_assertions)
                && url.scheme() == "http"
                && matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1"));
        if !valid_scheme
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(LightToolPushDeliveryError::InvalidEndpoint);
        }

        Ok(())
    }
}

fn canonical_host(host: &str) -> String {
    host.trim().trim_end_matches('.').to_ascii_lowercase()
}
