//! Tests for the id-verifier that post-processes LLM responses.
//!
//! The verifier enforces the `[id=N]` citation contract: the model is
//! instructed (via system prompt + tool description) to cite `[id=N]`
//! after every item it mentions, and this module catches cases where
//! the model fabricates an id that no tool call returned. Any line
//! containing at least one unverified id is stripped entirely and a
//! short user-visible footer is appended so the user knows something
//! was redacted.

use backend::utils::id_verifier::{collect_tool_result_ids, verify_and_strip, STRIPPED_FOOTER};
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

#[test]
fn verify_strips_id_tags_from_surviving_lines() {
    // All ids valid → no line dropped, but the `[id=N]` tags
    // themselves are silently removed so the user never sees our
    // internal citation plumbing in the final SMS.
    let valid: HashSet<i64> = [101, 102].into_iter().collect();
    let resp =
        "You have 2 messages:\n1. Alice - quarterly review [id=101]\n2. Bob - sounds good [id=102]";
    let (out, dropped) = verify_and_strip(resp, &valid);
    assert!(!dropped, "no line was dropped for verification");
    assert!(
        !out.contains("[id="),
        "all id tags must be removed from final output, got: {out:?}"
    );
    assert!(out.contains("Alice"));
    assert!(out.contains("Bob"));
    assert!(out.contains("quarterly review"));
    assert!(out.contains("sounds good"));
    // Footer should NOT appear — nothing was dropped.
    assert!(!out.contains(STRIPPED_FOOTER));
}

#[test]
fn verify_trims_dangling_separator_left_behind_by_tag_removal() {
    // When a line ends with " - [id=N]" or " — [id=N]", removing the
    // tag must not leave a dangling punctuation artifact.
    let valid: HashSet<i64> = [101].into_iter().collect();
    let resp = "1. Alice — [id=101]";
    let (out, _dropped) = verify_and_strip(resp, &valid);
    assert!(!out.contains("[id="));
    assert!(out.contains("Alice"));
    for line in out.lines() {
        let last_char = line.chars().last();
        assert!(
            !matches!(last_char, Some('—') | Some('-') | Some('–') | Some(':')),
            "line '{line}' has dangling separator after tag removal"
        );
    }
}

#[test]
fn verify_drops_line_with_unknown_id_and_appends_footer() {
    let valid: HashSet<i64> = [101].into_iter().collect();
    let resp = "You have 2 messages:\n1. Alice [id=101]\n2. Phantom [id=999]";
    let (out, dropped) = verify_and_strip(resp, &valid);
    assert!(dropped);
    assert!(out.contains("Alice"));
    assert!(!out.contains("[id="), "id tags must be stripped: {out:?}");
    assert!(!out.contains("Phantom"));
    assert!(!out.contains("999"));
    assert!(out.contains(STRIPPED_FOOTER));
}

#[test]
fn verify_drops_line_even_when_one_id_is_valid() {
    // A line citing both a real id AND a fake id is dropped wholesale
    // — we can't trust any part of it because the model mixed fact
    // and fabrication in one sentence.
    let valid: HashSet<i64> = [101].into_iter().collect();
    let resp = "1. See [id=101] and [id=999] together";
    let (out, dropped) = verify_and_strip(resp, &valid);
    assert!(dropped);
    assert!(!out.contains("See"));
    assert!(!out.contains("[id="));
    assert!(out.contains(STRIPPED_FOOTER));
}

#[test]
fn verify_keeps_lines_without_ids() {
    // Headers, counts, and prose lines without `[id=N]` references
    // are not in scope for verification and pass through unchanged.
    // Surviving lines WITH ids get their tags stripped.
    let valid: HashSet<i64> = [101].into_iter().collect();
    let resp =
        "Summary for today:\n\n1. Alice - review [id=101]\n2. Phantom [id=999]\n\nReply to dig in.";
    let (out, dropped) = verify_and_strip(resp, &valid);
    assert!(dropped);
    assert!(out.contains("Summary for today:"));
    assert!(out.contains("Alice"));
    assert!(out.contains("review"));
    assert!(!out.contains("[id="), "id tags must be stripped: {out:?}");
    assert!(!out.contains("Phantom"));
    assert!(out.contains("Reply to dig in."));
    assert!(out.contains(STRIPPED_FOOTER));
}

#[test]
fn verify_empty_response_with_all_invalid_becomes_footer_only() {
    let valid: HashSet<i64> = HashSet::new();
    let resp = "1. Phantom [id=999]";
    let (out, dropped) = verify_and_strip(resp, &valid);
    assert!(dropped);
    // Footer alone — no preceding whitespace.
    assert_eq!(out, STRIPPED_FOOTER);
}

#[test]
fn verify_no_tool_calls_means_any_id_is_unverified() {
    // When no tools were called, `valid_ids` is empty and any cited
    // id must be dropped as fabrication.
    let valid: HashSet<i64> = HashSet::new();
    let resp = "1. Totally real thing [id=42]";
    let (out, dropped) = verify_and_strip(resp, &valid);
    assert!(dropped);
    assert!(!out.contains("[id="));
    assert!(!out.contains("Totally real thing"));
}

#[test]
fn verify_collapses_triple_newlines_after_dropping() {
    // When dropping a line from a previously well-spaced response,
    // we shouldn't leave a run of 3+ blank lines behind.
    let valid: HashSet<i64> = [101].into_iter().collect();
    let resp = "Line A [id=101]\n\nLine B [id=999]\n\nLine C [id=101]";
    let (out, dropped) = verify_and_strip(resp, &valid);
    assert!(dropped);
    assert!(!out.contains("\n\n\n"));
    assert!(!out.contains("[id="));
}

#[test]
fn verify_no_ids_anywhere_is_passthrough() {
    let valid: HashSet<i64> = HashSet::new();
    let resp = "Hello, how are you today?";
    let (out, dropped) = verify_and_strip(resp, &valid);
    assert!(!dropped);
    assert_eq!(out, resp);
}
