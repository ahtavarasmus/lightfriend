use backend::utils::sms_sanitizer::apply_sms_url_filter;

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
