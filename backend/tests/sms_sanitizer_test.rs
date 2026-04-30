use backend::utils::sms_sanitizer::{apply_sms_url_filter, unfang_text};

#[test]
fn preserves_text_without_urls() {
    let input = "Reminder: meeting at 3pm. Don't forget to bring notes.";
    assert_eq!(apply_sms_url_filter(input), input);
}

#[test]
fn preserves_trusted_domain() {
    let input = "Log in at https://lightfriend.ai/dashboard to continue.";
    assert_eq!(apply_sms_url_filter(input), input);
}

#[test]
fn preserves_trusted_subdomain() {
    let input = "See https://app.lightfriend.ai/settings for options.";
    assert_eq!(apply_sms_url_filter(input), input);
}

#[test]
fn replaces_third_party_url_with_domain_placeholder() {
    let input = "Article: https://en.wikipedia.org/wiki/Rust_programming";
    assert_eq!(
        apply_sms_url_filter(input),
        "Article: [link: en.wikipedia.org]"
    );
}

#[test]
fn replaces_shortener_with_bare_link() {
    let input = "Check it: https://bit.ly/3xYzAbC";
    assert_eq!(apply_sms_url_filter(input), "Check it: [link]");
}

#[test]
fn keeps_trailing_punctuation_outside_url() {
    let input = "Source: https://nytimes.com/article/foo.";
    assert_eq!(apply_sms_url_filter(input), "Source: [link: nytimes.com].");
}

#[test]
fn handles_multiple_urls_in_one_message() {
    let input = "First https://github.com/user/repo and second https://bit.ly/abc done.";
    assert_eq!(
        apply_sms_url_filter(input),
        "First [link: github.com] and second [link] done."
    );
}

#[test]
fn handles_phishing_shaped_url() {
    let input = "Verify at https://random-site.example/login?token=abc123";
    assert_eq!(
        apply_sms_url_filter(input),
        "Verify at [link: random-site.example]"
    );
}

#[test]
fn handles_http_scheme() {
    let input = "Visit http://example.com/path";
    assert_eq!(apply_sms_url_filter(input), "Visit [link: example.com]");
}

#[test]
fn handles_url_with_port() {
    let input = "Service at https://example.com:8080/api";
    assert_eq!(
        apply_sms_url_filter(input),
        "Service at [link: example.com]"
    );
}

#[test]
fn empty_string_is_unchanged() {
    assert_eq!(apply_sms_url_filter(""), "");
}

#[test]
fn url_at_end_of_sentence_keeps_punctuation() {
    let input = "More info on https://example.com!";
    assert_eq!(
        apply_sms_url_filter(input),
        "More info on [link: example.com]!"
    );
}

#[test]
fn unfang_dots_in_brackets() {
    assert_eq!(unfang_text("example[.]com"), "example.com");
    assert_eq!(unfang_text("foo[.]bar[.]baz"), "foo.bar.baz");
}

#[test]
fn unfang_at_in_brackets() {
    assert_eq!(unfang_text("user(at)example.com"), "user@example.com");
    assert_eq!(unfang_text("user[at]example.com"), "user@example.com");
    assert_eq!(unfang_text("user[@]example.com"), "user@example.com");
}

#[test]
fn unfang_dot_word_in_brackets() {
    assert_eq!(unfang_text("example(dot)com"), "example.com");
    assert_eq!(unfang_text("example[DOT]com"), "example.com");
}

#[test]
fn unfang_real_phishing_shaped_message() {
    // The actual message that got our number banned
    let input = "Google Security: New sign-in detected on an Apple iPhone for moon[.]gentile(at)gmail[.]com. Check activity if this wasn't you";
    let output = unfang_text(input);
    assert_eq!(output, "Google Security: New sign-in detected on an Apple iPhone for moon.gentile@gmail.com. Check activity if this wasn't you");
}

#[test]
fn unfang_does_not_touch_innocent_prose() {
    assert_eq!(
        unfang_text("Meet me (at) the cafe by 5pm."),
        "Meet me (at) the cafe by 5pm."
    );
    assert_eq!(
        unfang_text("Press [Enter] to continue."),
        "Press [Enter] to continue."
    );
    assert_eq!(unfang_text(""), "");
}

#[test]
fn full_sanitizer_unfangs_then_filters_urls() {
    let input = "See https://example[.]com/login and email user(at)gmail.com";
    let output = apply_sms_url_filter(input);
    // The URL gets unfanged, then domain-replaced. Email is left in place.
    assert_eq!(output, "See [link: example.com] and email user@gmail.com");
}
