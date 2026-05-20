//! Thin embedding client for commitment similarity memory.
//!
//! Hits an OpenAI-compatible `/v1/embeddings` endpoint via the same Tinfoil /
//! OpenRouter setup the rest of the AI pipeline uses. Defaults to Tinfoil's
//! `nomic-embed-text` so user content stays inside the same verified-inference
//! trust boundary as the chat completion path. Override via env:
//!
//!   COMMITMENT_EMBEDDING_PROVIDER  one of "tinfoil" | "openrouter" | "none"
//!                                  (default: "tinfoil")
//!   COMMITMENT_EMBEDDING_MODEL     model name passed to /v1/embeddings
//!                                  (default: "nomic-embed-text" on tinfoil,
//!                                  required for non-tinfoil providers)
//!
//! Setting `COMMITMENT_EMBEDDING_PROVIDER=none` disables the feature -
//! `generate_embedding` returns `Ok(None)` and callers gracefully skip the
//! similarity check.

use openai_api_rs::v1::embedding::EmbeddingRequest;

use crate::AiConfig;

/// Tinfoil's verified-inference embedding model. Selected as default because
/// it's open-weight (nomic-ai/nomic-embed-text-v1.5), produces compact 768-d
/// vectors, and runs in the same Tinfoil trust boundary as the chat models.
const DEFAULT_TINFOIL_EMBEDDING_MODEL: &str = "nomic-embed-text";

enum ProviderChoice {
    Tinfoil,
    OpenRouter,
    Disabled,
}

/// Resolve the embedding provider from env. Falls back to Tinfoil.
fn provider_from_env() -> ProviderChoice {
    match std::env::var("COMMITMENT_EMBEDDING_PROVIDER")
        .ok()
        .as_deref()
        .map(str::trim)
    {
        Some("none") => ProviderChoice::Disabled,
        Some("openrouter") => ProviderChoice::OpenRouter,
        _ => ProviderChoice::Tinfoil,
    }
}

/// Generate an embedding for `text`. Returns `Ok(None)` when the feature is
/// explicitly disabled or no model can be resolved (e.g. a non-tinfoil
/// provider without `COMMITMENT_EMBEDDING_MODEL` set) so callers can branch
/// off without an error path. Returns `Err` only when the provider call
/// actually fails - the caller should log and continue without similarity.
pub async fn generate_embedding(
    ai_config: &AiConfig,
    text: &str,
) -> Result<Option<Vec<f32>>, String> {
    let provider = match provider_from_env() {
        ProviderChoice::Disabled => return Ok(None),
        ProviderChoice::Tinfoil => crate::AiProvider::Tinfoil,
        ProviderChoice::OpenRouter => crate::AiProvider::OpenRouter,
    };

    let env_model = std::env::var("COMMITMENT_EMBEDDING_MODEL")
        .ok()
        .filter(|m| !m.trim().is_empty());

    let model = match (env_model, provider) {
        (Some(m), _) => m,
        (None, crate::AiProvider::Tinfoil) => DEFAULT_TINFOIL_EMBEDDING_MODEL.to_string(),
        (None, _) => return Ok(None),
    };
    let client = ai_config
        .create_client(provider)
        .map_err(|e| format!("embedding client build failed: {}", e))?;

    let req = EmbeddingRequest::new(model, vec![text.to_string()]);

    let response = client
        .embedding(req)
        .await
        .map_err(|e| format!("embedding request failed: {}", e))?;

    let embedding = response
        .data
        .into_iter()
        .next()
        .map(|d| d.embedding)
        .ok_or_else(|| "embedding response had no data".to_string())?;

    Ok(Some(embedding))
}
