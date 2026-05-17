//! Incoming SMS handler for commitment-prompt replies (1/2/3/4).
//!
//! When `run_commitment_detection` sends an SMS prompt, it writes a row to
//! `commitment_prompts`. This module parses the user's numeric reply and
//! applies the corresponding action: create a tracked event (1, 2), upsert a
//! sender rule (2, 3), or just record the negative label (4). Returns
//! `Some(confirmation)` when a reply was handled so the caller can short-
//! circuit the regular agent processing path.
//!
//! 1 and 2 also seed a `track` content embedding (for cross-sender rescue of
//! future missed commitments). 4 seeds a `wrong` embedding (for suppressing
//! near-duplicate false positives). 3 only sets a sender rule - it's a
//! sender preference, not a content signal.

use std::sync::Arc;

use tracing::{info, warn};

use crate::models::commitment_models::{NewCommitmentLabelEmbedding, NewCommitmentSenderRule};
use crate::models::ontology_models::NewOntEvent;
use crate::models::user_models::User;
use crate::repositories::commitment_repository::{
    LABEL_TRACK, LABEL_WRONG, REPLY_ALWAYS, REPLY_MUTE, REPLY_TRACK, REPLY_WRONG,
    RULE_ALWAYS_TRACK, RULE_MUTE, RULE_SOURCE_SMS,
};
use crate::utils::{embedding_service, embedding_similarity};
use crate::AppState;

/// Inspect an incoming SMS body. If it's a valid commitment-prompt reply
/// (1/2/3/4) AND the user has an unresolved prompt, apply the action and
/// return a confirmation message. Returns `None` if the body isn't a
/// commitment reply or there's no pending prompt - caller should fall back
/// to regular SMS processing.
pub async fn try_handle_reply(state: &Arc<AppState>, user: &User, body: &str) -> Option<String> {
    let reply = parse_reply(body)?;

    let prompt = match state
        .commitment_repository
        .find_latest_unresolved_for_user(user.id)
    {
        Ok(Some(p)) => p,
        Ok(None) => {
            info!(
                "commitment_reply user={} got '{}' but no pending prompt - ignoring as agent input",
                user.id, body
            );
            return None;
        }
        Err(e) => {
            warn!(
                "commitment_reply user={} pending lookup failed: {}",
                user.id, e
            );
            return None;
        }
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i32;

    let confirmation = match reply {
        REPLY_TRACK => apply_track(state, &prompt, now).await,
        REPLY_ALWAYS => apply_always(state, &prompt, now).await,
        REPLY_MUTE => apply_mute(state, &prompt),
        REPLY_WRONG => apply_wrong(state, &prompt).await,
        _ => unreachable!(),
    };

    Some(confirmation)
}

/// Strict parser: leading whitespace tolerated, but the body must be exactly
/// "1", "2", "3", or "4" otherwise we don't treat it as a commitment reply.
/// This keeps general SMS chat ("1 thing I noticed today...") from being
/// hijacked as a numeric label.
fn parse_reply(body: &str) -> Option<&'static str> {
    match body.trim() {
        "1" => Some(REPLY_TRACK),
        "2" => Some(REPLY_ALWAYS),
        "3" => Some(REPLY_MUTE),
        "4" => Some(REPLY_WRONG),
        _ => None,
    }
}

/// Conservative fallback when the user replied within the TTL window but
/// the prompt was already resolved (duplicate Twilio webhook, racing
/// retry). The user still gets an acknowledgement instead of the bare SMS
/// reply silently leaking into the regular agent flow.
const ALREADY_PROCESSED: &str = "Already handled that prompt.";

/// Try to atomically claim the prompt for the given reply label. Returns
/// `true` when the caller is the first to resolve it - only then should
/// side effects run. `false` means a duplicate (Twilio retry, racing
/// reply) and the caller must return the already-handled message without
/// touching events / rules / embeddings.
fn claim_or_log_duplicate(
    state: &Arc<AppState>,
    prompt: &crate::models::commitment_models::CommitmentPrompt,
    reply: &str,
) -> bool {
    match state.commitment_repository.claim_prompt(prompt.id, reply) {
        Ok(1) => true,
        Ok(_) => {
            info!(
                "commitment_reply user={} reply={} duplicate (prompt {} already resolved)",
                prompt.user_id, reply, prompt.id
            );
            false
        }
        Err(e) => {
            warn!(
                "commitment_reply user={} reply={} claim_prompt failed: {}",
                prompt.user_id, reply, e
            );
            // Conservative: treat DB error as "do not double-apply".
            false
        }
    }
}

async fn apply_track(
    state: &Arc<AppState>,
    prompt: &crate::models::commitment_models::CommitmentPrompt,
    now: i32,
) -> String {
    if !claim_or_log_duplicate(state, prompt, REPLY_TRACK) {
        return ALREADY_PROCESSED.to_string();
    }

    let new_event = NewOntEvent {
        user_id: prompt.user_id,
        description: prompt.commitment_description.clone(),
        remind_at: prompt.remind_at,
        due_at: prompt.due_at,
        status: "active".to_string(),
        created_at: now,
        updated_at: now,
    };

    match state.ontology_repository.create_event(&new_event) {
        Ok(created) => {
            let _ = state.ontology_repository.create_link(
                prompt.user_id,
                "Event",
                created.id,
                "Message",
                prompt.ont_message_id as i32,
                "source_message",
                None,
            );
            if let Err(e) = state
                .commitment_repository
                .set_prompt_event_id(prompt.id, created.id)
            {
                warn!(
                    "commitment_reply set_prompt_event_id failed prompt={}: {}",
                    prompt.id, e
                );
            }
            seed_label_embedding(state, prompt, LABEL_TRACK, now).await;
            info!(
                "commitment_reply user={} reply=1 created event={} from prompt={}",
                prompt.user_id, created.id, prompt.id
            );
            format!("Tracking: {}", prompt.commitment_description)
        }
        Err(e) => {
            warn!(
                "commitment_reply user={} reply=1 create_event failed: {}",
                prompt.user_id, e
            );
            "Sorry, couldn't track that right now. Try again later.".to_string()
        }
    }
}

async fn apply_always(
    state: &Arc<AppState>,
    prompt: &crate::models::commitment_models::CommitmentPrompt,
    now: i32,
) -> String {
    if !claim_or_log_duplicate(state, prompt, REPLY_ALWAYS) {
        return ALREADY_PROCESSED.to_string();
    }

    let new_event = NewOntEvent {
        user_id: prompt.user_id,
        description: prompt.commitment_description.clone(),
        remind_at: prompt.remind_at,
        due_at: prompt.due_at,
        status: "active".to_string(),
        created_at: now,
        updated_at: now,
    };

    let created_id = match state.ontology_repository.create_event(&new_event) {
        Ok(created) => {
            let _ = state.ontology_repository.create_link(
                prompt.user_id,
                "Event",
                created.id,
                "Message",
                prompt.ont_message_id as i32,
                "source_message",
                None,
            );
            Some(created.id)
        }
        Err(e) => {
            warn!(
                "commitment_reply user={} reply=2 create_event failed: {}",
                prompt.user_id, e
            );
            None
        }
    };

    if let Some(eid) = created_id {
        if let Err(e) = state
            .commitment_repository
            .set_prompt_event_id(prompt.id, eid)
        {
            warn!(
                "commitment_reply set_prompt_event_id failed prompt={}: {}",
                prompt.id, e
            );
        }
    }

    // Upsert the always-track sender rule even if event create failed - the
    // user's preference signal is still valid for future detections.
    let rule = NewCommitmentSenderRule {
        user_id: prompt.user_id,
        platform: prompt.platform.clone(),
        sender_key: prompt.sender_key.clone(),
        rule_type: RULE_ALWAYS_TRACK.to_string(),
        source: RULE_SOURCE_SMS.to_string(),
        active: true,
        created_at: now,
    };
    if let Err(e) = state.commitment_repository.deactivate_existing_rules(
        prompt.user_id,
        &prompt.platform,
        &prompt.sender_key,
    ) {
        warn!(
            "commitment_reply user={} reply=2 deactivate_existing_rules failed: {}",
            prompt.user_id, e
        );
    }
    if let Err(e) = state.commitment_repository.create_sender_rule(&rule) {
        warn!(
            "commitment_reply user={} reply=2 create_sender_rule failed: {}",
            prompt.user_id, e
        );
    }

    seed_label_embedding(state, prompt, LABEL_TRACK, now).await;

    info!(
        "commitment_reply user={} reply=2 created event={:?} always_track_sender={} prompt={}",
        prompt.user_id, created_id, prompt.sender_key, prompt.id
    );

    format!(
        "Tracking: {} - will auto-track future from {}",
        prompt.commitment_description, prompt.sender_display_name
    )
}

fn apply_mute(
    state: &Arc<AppState>,
    prompt: &crate::models::commitment_models::CommitmentPrompt,
) -> String {
    if !claim_or_log_duplicate(state, prompt, REPLY_MUTE) {
        return ALREADY_PROCESSED.to_string();
    }

    if let Err(e) = state.commitment_repository.upsert_sender_rule(
        prompt.user_id,
        &prompt.platform,
        &prompt.sender_key,
        RULE_MUTE,
        RULE_SOURCE_SMS,
    ) {
        warn!(
            "commitment_reply user={} reply=3 upsert_sender_rule failed: {}",
            prompt.user_id, e
        );
    }

    info!(
        "commitment_reply user={} reply=3 muted sender={} prompt={}",
        prompt.user_id, prompt.sender_key, prompt.id
    );

    format!(
        "Muted {}. No more commitment prompts from them.",
        prompt.sender_display_name
    )
}

async fn apply_wrong(
    state: &Arc<AppState>,
    prompt: &crate::models::commitment_models::CommitmentPrompt,
) -> String {
    if !claim_or_log_duplicate(state, prompt, REPLY_WRONG) {
        return ALREADY_PROCESSED.to_string();
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i32;
    seed_label_embedding(state, prompt, LABEL_WRONG, now).await;

    info!(
        "commitment_reply user={} reply=4 wrong prompt={}",
        prompt.user_id, prompt.id
    );

    "Got it - won't treat this as a commitment.".to_string()
}

/// Fetch the original message content and store an embedding tagged with
/// `label_type`. Best-effort: any failure (no message, embedding service
/// disabled or errored) is logged and swallowed - the user-visible action
/// has already succeeded.
async fn seed_label_embedding(
    state: &Arc<AppState>,
    prompt: &crate::models::commitment_models::CommitmentPrompt,
    label_type: &str,
    now: i32,
) {
    let messages = match state
        .ontology_repository
        .get_messages_by_ids(&[prompt.ont_message_id])
    {
        Ok(m) => m,
        Err(e) => {
            warn!(
                "seed_label_embedding user={} get_messages failed: {}",
                prompt.user_id, e
            );
            return;
        }
    };
    let msg = match messages.into_iter().next() {
        Some(m) => m,
        None => {
            warn!(
                "seed_label_embedding user={} ont_message {} not found",
                prompt.user_id, prompt.ont_message_id
            );
            return;
        }
    };

    let embedding =
        match embedding_service::generate_embedding(&state.ai_config, &msg.content).await {
            Ok(Some(v)) => v,
            Ok(None) => return, // feature disabled - silent no-op
            Err(e) => {
                warn!(
                    "seed_label_embedding user={} embedding failed: {}",
                    prompt.user_id, e
                );
                return;
            }
        };

    let row = NewCommitmentLabelEmbedding {
        user_id: prompt.user_id,
        label_type: label_type.to_string(),
        embedding: embedding_similarity::pack_embedding(&embedding),
        source_message_id: Some(prompt.ont_message_id),
        created_at: now,
    };

    if let Err(e) = state.commitment_repository.store_label_embedding(&row) {
        warn!(
            "seed_label_embedding user={} store failed: {}",
            prompt.user_id, e
        );
    }
}
