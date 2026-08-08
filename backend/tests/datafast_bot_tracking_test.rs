use backend::utils::datafast_bot_tracking::{is_likely_bot, is_trackable_public_path};

#[test]
fn recognizes_common_ai_crawlers_case_insensitively() {
    assert!(is_likely_bot(
        "Mozilla/5.0 AppleWebKit/537.36; compatible; GPTBot/1.2"
    ));
    assert!(is_likely_bot("ClaudeBot/1.0"));
    assert!(is_likely_bot("PerplexityBot/1.0"));
    assert!(is_likely_bot("Google-Extended"));
    assert!(!is_likely_bot(
        "Mozilla/5.0 AppleWebKit/537.36 Chrome/127.0 Safari/537.36"
    ));
}

#[test]
fn tracks_public_content_and_crawler_files() {
    for path in [
        "/",
        "/pricing",
        "/blog/an-interesting-post",
        "/blog/md/an-interesting-post",
        "/robots.txt",
        "/sitemap.xml",
        "/llms.txt",
        "/assets/llm.txt",
    ] {
        assert!(
            is_trackable_public_path(path),
            "expected {path} to be tracked"
        );
    }
}
#[test]
fn skips_private_routes_and_static_assets() {
    for path in [
        "/api/profile",
        "/uploads/private.png",
        "/assets/styles.css",
        "/admin",
        "/billing/invoices",
        "/login",
        "/password-reset/private-token",
        "/set-password/private-token",
        "/subscription-success",
        "/.well-known/appspecific/key.pem",
        "/frontend.js",
        "/frontend_bg.wasm",
        "/favicon.png",
    ] {
        assert!(
            !is_trackable_public_path(path),
            "expected {path} to be skipped"
        );
    }
}
