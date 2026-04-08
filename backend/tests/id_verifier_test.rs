//! Tests for the id-verifier that post-processes LLM responses.
//!
//! The verifier enforces the `[id=N]` citation contract. It returns
//! two parallel versions of each response:
//!   - `user_facing`: what gets sent over SMS. `[id=N]` tags stripped,
//!     footer appended if any line was dropped for failing verification.
//!   - `history`: what gets stored in conversation history. `[id=N]`
//!     tags preserved so the LLM sees its own correctly-formatted
//!     prior turn next time and keeps citing ids.

use backend::utils::id_verifier::{collect_tool_result_ids, verify, STRIPPED_FOOTER};
use openai_api_rs::v1::chat_completion::{ChatCompletionMessage, Content, MessageRole};
use std::collections::HashSet;

fn tool_msg(text: &str) -> ChatCompletionMessage {
    ChatCompletionMessage {
        role: MessageRole::tool,
        content: Content::Text(text.to_string()),
        name: None,
        tool_calls: None,
        tool_call_id: Some("call_test".to_string()),
    }
}

fn user_msg(text: &str) -> ChatCompletionMessage {
    ChatCompletionMessage {
        role: MessageRole::user,
        content: Content::Text(text.to_string()),
        name: None,
        tool_calls: None,
        tool_call_id: None,
    }
}

#[test]
fn collect_ids_extracts_from_tool_messages_only() {
    // Only tool-role messages are trusted. A user turn mentioning an
    // id (e.g. echoing back a previous digest) must NOT be treated as
    // a valid citation source.
    let msgs = vec![
        user_msg("User said something mentioning [id=999]"),
        tool_msg("Found 2 messages:\n1. [id=101] via email\n2. [id=102] via whatsapp"),
        tool_msg("[id=103] and [id=104]"),
    ];
    let ids = collect_tool_result_ids(&msgs);
    assert_eq!(ids.len(), 4);
    assert!(ids.contains(&101));
    assert!(ids.contains(&102));
    assert!(ids.contains(&103));
    assert!(ids.contains(&104));
    assert!(!ids.contains(&999));
}

// =============================================================================
// user_facing output: tag stripping
// =============================================================================

#[test]
fn verify_strips_id_tags_from_user_facing_when_all_valid() {
    // All ids valid → no line dropped, but the `[id=N]` tags
    // themselves are silently removed from user_facing so the user
    // never sees our internal citation plumbing.
    let valid: HashSet<i64> = [101, 102].into_iter().collect();
    let resp =
        "You have 2 messages:\n1. Alice - quarterly review [id=101]\n2. Bob - sounds good [id=102]";
    let v = verify(resp, &valid);
    assert!(!v.dropped_line, "no line was dropped for verification");
    assert!(
        !v.user_facing.contains("[id="),
        "all id tags must be removed from user_facing, got: {:?}",
        v.user_facing
    );
    assert!(v.user_facing.contains("Alice"));
    assert!(v.user_facing.contains("Bob"));
    assert!(v.user_facing.contains("quarterly review"));
    assert!(v.user_facing.contains("sounds good"));
    // Footer should NOT appear — nothing was dropped.
    assert!(!v.user_facing.contains(STRIPPED_FOOTER));
}

#[test]
fn verify_trims_dangling_separator_after_tag_removal_in_user_facing() {
    // When a line ends with " - [id=N]" or " — [id=N]", removing the
    // tag must not leave a dangling punctuation artifact.
    let valid: HashSet<i64> = [101].into_iter().collect();
    let resp = "1. Alice — [id=101]";
    let v = verify(resp, &valid);
    assert!(!v.user_facing.contains("[id="));
    assert!(v.user_facing.contains("Alice"));
    for line in v.user_facing.lines() {
        let last_char = line.chars().last();
        assert!(
            !matches!(last_char, Some('—') | Some('-') | Some('–') | Some(':')),
            "line '{line}' has dangling separator after tag removal"
        );
    }
}

// =============================================================================
// history output: tags KEPT so the LLM sees its own citations next turn
// =============================================================================

#[test]
fn verify_history_preserves_tags_when_all_valid() {
    let valid: HashSet<i64> = [101, 102].into_iter().collect();
    let resp = "1. Alice [id=101]\n2. Bob [id=102]";
    let v = verify(resp, &valid);
    assert!(!v.dropped_line);
    // history must contain both tags verbatim — this is what the LLM
    // sees on the next turn as its own prior response.
    assert!(v.history.contains("[id=101]"));
    assert!(v.history.contains("[id=102]"));
    // history must NOT contain the footer — that's system noise not
    // written by the LLM.
    assert!(!v.history.contains(STRIPPED_FOOTER));
}

#[test]
fn verify_history_drops_invalid_lines_same_as_user_facing() {
    // Dropped lines are dropped from BOTH versions — we don't want
    // the LLM seeing its own fabrications in history either,
    // otherwise it might reuse the fake ids next turn.
    let valid: HashSet<i64> = [101].into_iter().collect();
    let resp = "1. Alice [id=101]\n2. Phantom [id=999]";
    let v = verify(resp, &valid);
    assert!(v.dropped_line);
    assert!(v.history.contains("[id=101]"));
    assert!(v.history.contains("Alice"));
    assert!(!v.history.contains("[id=999]"));
    assert!(!v.history.contains("Phantom"));
    // Footer is user-only, not in history.
    assert!(!v.history.contains(STRIPPED_FOOTER));
}

// =============================================================================
// Line-dropping + footer (user_facing only)
// =============================================================================

#[test]
fn verify_drops_line_with_unknown_id_and_appends_footer_to_user_facing() {
    let valid: HashSet<i64> = [101].into_iter().collect();
    let resp = "You have 2 messages:\n1. Alice [id=101]\n2. Phantom [id=999]";
    let v = verify(resp, &valid);
    assert!(v.dropped_line);
    assert!(v.user_facing.contains("Alice"));
    assert!(
        !v.user_facing.contains("[id="),
        "id tags must be stripped from user_facing: {:?}",
        v.user_facing
    );
    assert!(!v.user_facing.contains("Phantom"));
    assert!(!v.user_facing.contains("999"));
    assert!(v.user_facing.contains(STRIPPED_FOOTER));
}

#[test]
fn verify_drops_line_even_when_one_id_is_valid() {
    // A line citing both a real id AND a fake id is dropped wholesale
    // — we can't trust any part of it because the model mixed fact
    // and fabrication in one sentence.
    let valid: HashSet<i64> = [101].into_iter().collect();
    let resp = "1. See [id=101] and [id=999] together";
    let v = verify(resp, &valid);
    assert!(v.dropped_line);
    assert!(!v.user_facing.contains("See"));
    assert!(!v.user_facing.contains("[id="));
    assert!(v.user_facing.contains(STRIPPED_FOOTER));
    // history also dropped it — and still has no tags left because
    // the whole line is gone.
    assert!(!v.history.contains("See"));
    assert!(!v.history.contains("[id=101]"));
    assert!(!v.history.contains("[id=999]"));
}

#[test]
fn verify_keeps_lines_without_ids() {
    // Headers, counts, and prose lines without `[id=N]` references
    // are not in scope for verification and pass through unchanged
    // in both versions.
    let valid: HashSet<i64> = [101].into_iter().collect();
    let resp =
        "Summary for today:\n\n1. Alice - review [id=101]\n2. Phantom [id=999]\n\nReply to dig in.";
    let v = verify(resp, &valid);
    assert!(v.dropped_line);

    // user_facing: header + CTA pass through, tag stripped, footer.
    assert!(v.user_facing.contains("Summary for today:"));
    assert!(v.user_facing.contains("Alice"));
    assert!(v.user_facing.contains("review"));
    assert!(!v.user_facing.contains("[id="));
    assert!(!v.user_facing.contains("Phantom"));
    assert!(v.user_facing.contains("Reply to dig in."));
    assert!(v.user_facing.contains(STRIPPED_FOOTER));

    // history: same filtering, but [id=101] preserved, no footer.
    assert!(v.history.contains("Summary for today:"));
    assert!(v.history.contains("[id=101]"));
    assert!(v.history.contains("Alice"));
    assert!(!v.history.contains("Phantom"));
    assert!(v.history.contains("Reply to dig in."));
    assert!(!v.history.contains(STRIPPED_FOOTER));
}

#[test]
fn verify_empty_response_with_all_invalid_becomes_footer_only_in_user_facing() {
    let valid: HashSet<i64> = HashSet::new();
    let resp = "1. Phantom [id=999]";
    let v = verify(resp, &valid);
    assert!(v.dropped_line);
    // user_facing: footer alone.
    assert_eq!(v.user_facing, STRIPPED_FOOTER);
    // history: empty string (no footer, nothing to preserve).
    assert_eq!(v.history, "");
}

#[test]
fn verify_no_tool_calls_means_any_id_is_unverified() {
    // When no tools were called, `valid_ids` is empty and any cited
    // id must be dropped as fabrication.
    let valid: HashSet<i64> = HashSet::new();
    let resp = "1. Totally real thing [id=42]";
    let v = verify(resp, &valid);
    assert!(v.dropped_line);
    assert!(!v.user_facing.contains("[id="));
    assert!(!v.user_facing.contains("Totally real thing"));
    assert!(!v.history.contains("[id=42]"));
    assert!(!v.history.contains("Totally real thing"));
}

#[test]
fn verify_collapses_triple_newlines_after_dropping() {
    // When dropping a line from a previously well-spaced response,
    // we shouldn't leave a run of 3+ blank lines behind in either
    // version.
    let valid: HashSet<i64> = [101].into_iter().collect();
    let resp = "Line A [id=101]\n\nLine B [id=999]\n\nLine C [id=101]";
    let v = verify(resp, &valid);
    assert!(v.dropped_line);
    assert!(!v.user_facing.contains("\n\n\n"));
    assert!(!v.user_facing.contains("[id="));
    assert!(!v.history.contains("\n\n\n"));
    assert!(v.history.contains("[id=101]"));
    assert!(!v.history.contains("[id=999]"));
}

#[test]
fn verify_no_ids_anywhere_is_passthrough() {
    // A response with zero `[id=N]` anywhere is treated as having no
    // data claims to verify — both versions equal the original.
    let valid: HashSet<i64> = HashSet::new();
    let resp = "Hello, how are you today?";
    let v = verify(resp, &valid);
    assert!(!v.dropped_line);
    assert_eq!(v.user_facing, resp);
    assert_eq!(v.history, resp);
}
