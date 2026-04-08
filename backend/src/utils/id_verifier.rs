//! Post-hoc verifier for LLM responses that cite ontology ids via the
//! `[id=N]` convention.
//!
//! Why this exists: the system prompt and tool descriptions instruct the
//! model to cite `[id=N]` for every item it mentions in a digest/summary,
//! so that any claim about the user's data can be traced back to a row
//! the tool actually returned. Without post-processing, this is an honor
//! system — a confused or adversarially-jailbroken model could still
//! output a fabricated `[id=99999]` for an email that doesn't exist.
//!
//! This module parses the model's final response for `[id=N]` tokens,
//! cross-checks each id against the set of ids returned by any tool
//! call in the current turn, and strips any line whose ids don't all
//! verify. If stripping happened, a short user-visible footer is
//! appended so the user knows something was redacted — which gives
//! them a natural hook to ask a follow-up and dig in.
//!
//! Deliberately silent: no tracing, no alerting, no logging. The user
//! asked for this to be invisible to ops and visible only to the user
//! whose answer got trimmed.

use openai_api_rs::v1::chat_completion::{ChatCompletionMessage, Content, MessageRole};
use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

/// Shared compiled regex — `[id=<digits>]` with no whitespace.
fn id_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[id=(\d+)\]").expect("static regex compiles"))
}

/// User-visible footer appended to a response when the verifier had to
/// strip at least one line containing an unverified id. `pub` so the
/// integration tests can assert against it directly.
/// Kept short — SMS chars are expensive.
pub const STRIPPED_FOOTER: &str = "(Stripped unverified items — ask to retry)";

/// Collect every `[id=N]` that appears in any tool-role message in the
/// current turn. These are the "trusted" ids the model is allowed to
/// cite in its final response — everything else is a fabrication.
pub fn collect_tool_result_ids(messages: &[ChatCompletionMessage]) -> HashSet<i64> {
    let re = id_re();
    let mut ids = HashSet::new();
    for msg in messages {
        if msg.role != MessageRole::tool {
            continue;
        }
        if let Content::Text(text) = &msg.content {
            for cap in re.captures_iter(text) {
                if let Ok(n) = cap[1].parse::<i64>() {
                    ids.insert(n);
                }
            }
        }
    }
    ids
}

/// Verify that every `[id=N]` cited in `response` appears in
/// `valid_ids`, then strip the `[id=N]` markers themselves from
/// surviving lines so the user never sees our internal citation
/// plumbing. Any line containing at least one unverified id is
/// dropped entirely (both verification and tag removal happen in one
/// pass). If anything was dropped, a user-visible footer is appended.
/// Lines with no ids at all are preserved as-is (counts, headers,
/// closing CTAs).
///
/// Returns `(verified_response, something_was_stripped_for_verification)`.
///
/// Note: the returned bool is ONLY true when a LINE was dropped for
/// failing verification. Stripping the `[id=N]` tags themselves from
/// kept lines is silent — the user should never know we were citing
/// ids in the first place.
pub fn verify_and_strip(response: &str, valid_ids: &HashSet<i64>) -> (String, bool) {
    let re = id_re();
    let mut dropped_line = false;

    let kept: Vec<String> = response
        .lines()
        .filter_map(|line| {
            // Collect all ids cited on this line.
            let ids_on_line: Vec<i64> = re
                .captures_iter(line)
                .filter_map(|cap| cap[1].parse::<i64>().ok())
                .collect();

            if ids_on_line.is_empty() {
                // No ids on this line — nothing to verify, pass through.
                return Some(line.to_string());
            }

            // Every id on this line must be in the trusted set.
            let all_ok = ids_on_line.iter().all(|id| valid_ids.contains(id));
            if !all_ok {
                dropped_line = true;
                return None;
            }

            // Line verified. Strip the `[id=N]` markers themselves
            // from the user-visible output — internal plumbing, not
            // noise the user should see. Also collapse any double
            // spaces or trailing spaces that removing tokens leaves
            // behind, and trim trailing punctuation artifacts like
            // " - ." or " —".
            let cleaned = re.replace_all(line, "");
            let collapsed = cleaned.split_whitespace().collect::<Vec<&str>>().join(" ");
            // Trim trailing dangling separators commonly left after
            // an id was at the end of the line (e.g. "Alice - " or
            // "Alice —").
            let trimmed = collapsed
                .trim_end_matches(|c: char| {
                    c.is_whitespace() || c == '-' || c == '—' || c == '–' || c == ':'
                })
                .to_string();
            Some(trimmed)
        })
        .collect();

    let mut result = kept.join("\n");

    // Clean up any run of 3+ newlines left behind by dropping sparse
    // lines from a previously well-spaced response.
    while result.contains("\n\n\n") {
        result = result.replace("\n\n\n", "\n\n");
    }
    result = result.trim_end().to_string();

    if dropped_line {
        if !result.is_empty() {
            result.push_str("\n\n");
        }
        result.push_str(STRIPPED_FOOTER);
    }

    (result, dropped_line)
}

// Tests live in `backend/tests/id_verifier_test.rs` — this repo keeps
// all tests as integration tests under `tests/` (no inline
// `#[cfg(test)] mod tests` in source files, per CLAUDE.md).
