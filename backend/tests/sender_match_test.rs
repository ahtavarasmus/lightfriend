//! Tests for `match_email_sender_to_person` — the pure helper that
//! decides which ontology Person an incoming email should be tagged
//! with, based on the envelope's `from` address and name.
//!
//! The cases here cover the two user-visible bugs we found:
//!
//! - Phishing / forwarding that spoofs the `From:` header to the
//!   user's own email address
//! - A Person erroneously saved with the user's own email as a
//!   channel handle (user error in the settings UI)
//!
//! plus the general hardening of requiring exact-match on email
//! handles instead of permissive substring containment.

use backend::handlers::imap_handlers::match_email_sender_to_person;
use backend::models::ontology_models::{OntChannel, OntPerson, PersonWithChannels};
use std::collections::HashSet;

fn person(id: i32, name: &str) -> OntPerson {
    OntPerson {
        id,
        user_id: 1,
        name: name.to_string(),
        created_at: 0,
        updated_at: 0,
    }
}

fn email_channel(id: i32, person_id: i32, handle: &str) -> OntChannel {
    OntChannel {
        id,
        user_id: 1,
        person_id,
        platform: "email".to_string(),
        handle: Some(handle.to_string()),
        room_id: None,
        notification_mode: "default".to_string(),
        notification_type: "sms".to_string(),
        notify_on_call: 1,
        created_at: 0,
    }
}

fn pwc(id: i32, name: &str, email_handle: &str) -> PersonWithChannels {
    PersonWithChannels {
        person: person(id, name),
        channels: vec![email_channel(id, id, email_handle)],
        edits: vec![],
    }
}

fn own(emails: &[&str]) -> HashSet<String> {
    emails.iter().map(|s| s.to_lowercase()).collect()
}

// =============================================================================
// Happy path
// =============================================================================

#[test]
fn matches_exact_email() {
    let persons = vec![pwc(1, "Alice", "alice@example.com")];
    let owns = own(&["me@mydomain.com"]);
    let id = match_email_sender_to_person(
        Some("alice@example.com"),
        Some("Alice Smith"),
        &persons,
        &owns,
    );
    assert_eq!(id, Some(1));
}

#[test]
fn email_match_is_case_insensitive() {
    let persons = vec![pwc(1, "Alice", "Alice@Example.COM")];
    let owns = HashSet::new();
    let id =
        match_email_sender_to_person(Some("alice@example.com"), Some("Alice"), &persons, &owns);
    assert_eq!(id, Some(1));
}

#[test]
fn no_match_when_persons_empty() {
    let persons: Vec<PersonWithChannels> = vec![];
    let owns = HashSet::new();
    let id =
        match_email_sender_to_person(Some("anyone@nowhere.com"), Some("Anyone"), &persons, &owns);
    assert_eq!(id, None);
}

// =============================================================================
// Self-sender guard: phishing spoof + forwarding
// =============================================================================

#[test]
fn refuses_to_match_when_sender_is_user_self_email() {
    // Classic phishing: "From:" header spoofed to the user's own
    // address. Our matcher must NOT attach this email to any Person,
    // even if the user mistakenly has a Person configured with their
    // own email as the channel handle.
    let persons = vec![pwc(1, "mommy", "rasmus@ahtava.com")];
    let owns = own(&["rasmus@ahtava.com"]);
    let id = match_email_sender_to_person(Some("rasmus@ahtava.com"), Some(""), &persons, &owns);
    assert_eq!(
        id, None,
        "self-email sender must not match any Person (phishing guard)"
    );
}

#[test]
fn refuses_to_match_when_sender_matches_one_of_multiple_own_emails() {
    // Users with multiple connected IMAP accounts should have all of
    // their own addresses considered "self" for this guard.
    let persons = vec![pwc(1, "mommy", "primary@me.com")];
    let owns = own(&["primary@me.com", "work@me.com", "backup@me.com"]);
    let id = match_email_sender_to_person(Some("work@me.com"), Some(""), &persons, &owns);
    assert_eq!(id, None);
}

// =============================================================================
// Self-handle guard: user misconfigured a Person with their own email
// =============================================================================

#[test]
fn skips_channel_whose_handle_is_user_own_email() {
    // User mistakenly saved a Person with their own email as the
    // channel handle. Even if the incoming email is genuinely from a
    // different contact, that misconfigured Person must never be
    // matched against any sender.
    let persons = vec![
        pwc(1, "mommy", "rasmus@ahtava.com"), // user's own email — bogus
        pwc(2, "alice", "alice@example.com"), // legitimate
    ];
    let owns = own(&["rasmus@ahtava.com"]);

    // Sender is Alice — mommy must be skipped, alice must match.
    let id =
        match_email_sender_to_person(Some("alice@example.com"), Some("Alice"), &persons, &owns);
    assert_eq!(id, Some(2));
}

// =============================================================================
// Email handle hardening: exact match, not substring
// =============================================================================

#[test]
fn email_handle_does_not_substring_match() {
    // Handle "bob@gmail.com" must NOT match "rob@gmail.com" even
    // though the bigger string technically contains the handle
    // (it doesn't, but this test guards against a possible future
    // refactor that goes back to `contains`).
    let persons = vec![pwc(1, "Bob", "bob@gmail.com")];
    let owns = HashSet::new();
    let id = match_email_sender_to_person(Some("rob@gmail.com"), Some("Rob"), &persons, &owns);
    assert_eq!(id, None);
}

#[test]
fn email_handle_does_not_match_longer_sender_containing_handle() {
    // A partial handle must never match a longer address. Regression
    // test for the original bug: from_email.contains(handle).
    let persons = vec![pwc(1, "Bob", "bob@gmail.com")];
    let owns = HashSet::new();
    let id =
        match_email_sender_to_person(Some("bob@gmail.com.evil.com"), Some(""), &persons, &owns);
    assert_eq!(
        id, None,
        "handle 'bob@gmail.com' must NOT substring-match 'bob@gmail.com.evil.com'"
    );
}

#[test]
fn domain_only_handle_does_not_match_every_sender_on_that_domain() {
    // Historical footgun: a user might try to save "gmail.com" as a
    // handle hoping to match anyone from gmail. The old code would
    // collapse every gmail user onto that one Person. New behavior:
    // the handle looks like an email (contains '@'? no here — so
    // it's treated as a name). But it's 9 chars so name match would
    // fire on any sender name containing "gmail.com". This test
    // documents that we DON'T match it against the from_email field.
    let persons = vec![pwc(1, "Gmail Bucket", "gmail.com")];
    let owns = HashSet::new();
    // Even though from_email contains "gmail.com" literally, handle
    // is treated as a name (no '@'), and name field doesn't contain it.
    let id = match_email_sender_to_person(
        Some("alice@gmail.com"),
        Some("Alice Smith"),
        &persons,
        &owns,
    );
    assert_eq!(id, None);
}

// =============================================================================
// Name handle match: substring on name field, length guard
// =============================================================================

#[test]
fn name_handle_substring_matches_display_name() {
    let persons = vec![pwc(1, "Alice", "Alice")]; // handle is a name
    let owns = HashSet::new();
    let id = match_email_sender_to_person(
        Some("alice.smith@work.com"),
        Some("Alice Smith"),
        &persons,
        &owns,
    );
    assert_eq!(id, Some(1));
}

#[test]
fn name_handle_shorter_than_three_chars_never_matches() {
    // "a" as a handle would match almost every sender name. We
    // require handles of at least 3 characters for name-substring
    // matching. This test documents that guard.
    let persons = vec![pwc(1, "Bucket", "a")];
    let owns = HashSet::new();
    let id = match_email_sender_to_person(
        Some("alice@example.com"),
        Some("Alice Smith"),
        &persons,
        &owns,
    );
    assert_eq!(id, None);
}

#[test]
fn name_handle_exactly_three_chars_is_allowed() {
    // Three chars is the minimum. "Ali" matches "Alice Smith".
    let persons = vec![pwc(1, "Alice", "Ali")];
    let owns = HashSet::new();
    let id = match_email_sender_to_person(
        Some("alice@example.com"),
        Some("Alice Smith"),
        &persons,
        &owns,
    );
    assert_eq!(id, Some(1));
}

// =============================================================================
// Empty / missing input
// =============================================================================

#[test]
fn empty_from_email_and_name_never_matches() {
    let persons = vec![pwc(1, "Alice", "alice@example.com")];
    let owns = HashSet::new();
    let id = match_email_sender_to_person(None, None, &persons, &owns);
    assert_eq!(id, None);
}

#[test]
fn empty_handle_is_skipped() {
    // A channel with handle = Some("") should not match anything.
    let mut p = pwc(1, "Alice", "");
    p.channels[0].handle = Some(String::new());
    let persons = vec![p];
    let owns = HashSet::new();
    let id =
        match_email_sender_to_person(Some("alice@example.com"), Some("Alice"), &persons, &owns);
    assert_eq!(id, None);
}

#[test]
fn channels_on_non_email_platforms_are_skipped() {
    // A whatsapp channel should never be considered when matching
    // an email sender, even if its handle happens to look like an
    // email.
    let persons = vec![PersonWithChannels {
        person: person(1, "Alice"),
        channels: vec![OntChannel {
            id: 10,
            user_id: 1,
            person_id: 1,
            platform: "whatsapp".to_string(),
            handle: Some("alice@example.com".to_string()),
            room_id: None,
            notification_mode: "default".to_string(),
            notification_type: "sms".to_string(),
            notify_on_call: 1,
            created_at: 0,
        }],
        edits: vec![],
    }];
    let owns = HashSet::new();
    let id =
        match_email_sender_to_person(Some("alice@example.com"), Some("Alice"), &persons, &owns);
    assert_eq!(id, None);
}
