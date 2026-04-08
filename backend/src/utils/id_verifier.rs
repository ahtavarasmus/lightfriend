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

/// Result of running the id verifier over an LLM response.
///
/// The verifier produces two parallel outputs from a single pass over
/// the response so that the user-facing version and the conversation-
/// history version can diverge on tag handling without a second walk.
///
/// Both versions share the same line filtering: any line containing
/// at least one `[id=N]` that isn't in the trusted set is dropped
/// from BOTH outputs. The difference is in what kept lines look like:
///
/// - `user_facing` has the `[id=N]` tags stripped (internal plumbing
///   the user should never see) and, if at least one line was
///   dropped, an explanatory footer appended.
/// - `history` keeps the `[id=N]` tags intact so that next turn the
///   LLM sees its own correctly-formatted prior answer in conversation
///   history and keeps citing ids. Without this, the model would
///   quickly "learn" from stripped history that citations are
///   optional and drift into uncited prose — which defeats the
///   verifier entirely on the next turn.
#[derive(Debug, Clone)]
pub struct VerifiedResponse {
    /// Send this to the user. `[id=N]` tags stripped, footer appended
    /// if any line was dropped for failing verification.
    pub user_facing: String,

    /// Store this in conversation history. `[id=N]` tags preserved
    /// exactly as the model wrote them on kept lines. No footer —
    /// footers are system messages, not the LLM's words.
    pub history: String,

    /// True if at least one line was dropped for failing verification.
    pub dropped_line: bool,
}

/// Verify every `[id=N]` cited in `response` against `valid_ids` and
/// produce both a user-facing and a history-bound version of the text.
/// See [`VerifiedResponse`] for the rationale behind keeping the two
/// versions distinct.
///
/// Invariants:
/// - Lines with no ids at all pass through both versions unchanged.
/// - A line where ANY cited id is not in `valid_ids` is dropped from
///   both versions (a line mixing real and fake ids can't be trusted
///   in parts).
/// - Kept lines' `[id=N]` tags are preserved in `history` and removed
///   in `user_facing`. Tag removal also trims dangling separators
///   ("Alice - ", "Alice —") so the user-facing line looks clean.
/// - Triple newlines left behind by dropping sparse lines are
///   collapsed to doubles in both versions.
/// - The footer is appended to `user_facing` only, and only when at
///   least one line was dropped.
pub fn verify(response: &str, valid_ids: &HashSet<i64>) -> VerifiedResponse {
    let re = id_re();
    let mut dropped_line = false;

    let mut history_lines: Vec<String> = Vec::new();
    let mut user_lines: Vec<String> = Vec::new();

    for line in response.lines() {
        let ids_on_line: Vec<i64> = re
            .captures_iter(line)
            .filter_map(|cap| cap[1].parse::<i64>().ok())
            .collect();

        if ids_on_line.is_empty() {
            // No ids on this line — pass through both versions.
            history_lines.push(line.to_string());
            user_lines.push(line.to_string());
            continue;
        }

        let all_ok = ids_on_line.iter().all(|id| valid_ids.contains(id));
        if !all_ok {
            dropped_line = true;
            continue; // drop from both
        }

        // History version keeps the line exactly as written so the
        // LLM sees its own [id=N] format on the next turn.
        history_lines.push(line.to_string());

        // User-facing version strips the tags and cleans up
        // dangling separators left behind.
        let cleaned = re.replace_all(line, "");
        let collapsed = cleaned.split_whitespace().collect::<Vec<&str>>().join(" ");
        let trimmed = collapsed
            .trim_end_matches(|c: char| {
                c.is_whitespace() || c == '-' || c == '—' || c == '–' || c == ':'
            })
            .to_string();
        user_lines.push(trimmed);
    }

    // Collapse 3+ blank lines left over from dropping, and trim the
    // trailing whitespace. Same transform for both versions — they
    // have the same line structure, only the content of kept lines
    // differs.
    fn collapse(lines: Vec<String>) -> String {
        let mut s = lines.join("\n");
        while s.contains("\n\n\n") {
            s = s.replace("\n\n\n", "\n\n");
        }
        s.trim_end().to_string()
    }

    let history = collapse(history_lines);
    let mut user_facing = collapse(user_lines);

    if dropped_line {
        if !user_facing.is_empty() {
            user_facing.push_str("\n\n");
        }
        user_facing.push_str(STRIPPED_FOOTER);
    }

    VerifiedResponse {
        user_facing,
        history,
        dropped_line,
    }
}

// Tests live in `backend/tests/id_verifier_test.rs` — this repo keeps
// all tests as integration tests under `tests/` (no inline
// `#[cfg(test)] mod tests` in source files, per CLAUDE.md).
