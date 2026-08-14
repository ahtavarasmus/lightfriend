use axum::body::to_bytes;
use axum::http::header;
use backend::blog::content::BlogStore;

fn load_blog() -> BlogStore {
    let content_dir = format!("{}/content/blog", env!("CARGO_MANIFEST_DIR"));
    BlogStore::load(&content_dir).expect("blog content should load")
}

#[test]
fn sitemap_contains_indexable_public_routes_and_blog_posts() {
    let store = load_blog();
    let sitemap = &store.sitemap_xml;

    for url in [
        "https://lightfriend.ai/",
        "https://lightfriend.ai/pricing",
        "https://lightfriend.ai/supported-countries",
        "https://lightfriend.ai/compatible-phones",
        "https://lightfriend.ai/how-it-works",
        "https://lightfriend.ai/can-i-leave-my-smartphone",
        "https://lightfriend.ai/limitations",
        "https://lightfriend.ai/privacy-architecture",
        "https://lightfriend.ai/ai-assistant-by-sms",
        "https://lightfriend.ai/email-on-dumbphone",
        "https://lightfriend.ai/whatsapp-on-dumbphone",
        "https://lightfriend.ai/mcp",
        "https://lightfriend.ai/bring-own-number",
        "https://lightfriend.ai/blog",
        "https://lightfriend.ai/blog/ai-assistant-via-sms",
        "https://lightfriend.ai/blog/intentional-notifications",
        "https://lightfriend.ai/blog/message-digests",
        "https://lightfriend.ai/blog/cross-service-urgency-context",
        "https://lightfriend.ai/blog/messages-without-a-smartphone",
        "https://lightfriend.ai/blog/keep-whatsapp-linked-device-connected",
        "https://lightfriend.ai/blog/send-messages-with-a-cancel-window",
        "https://lightfriend.ai/blog/when-a-connected-service-is-unavailable",
        "https://lightfriend.ai/blog/privacy-minded-minimal-assistant",
        "https://lightfriend.ai/blog/what-lightfriend-does-not-try-to-do",
        "https://lightfriend.ai/blog/best-dumbphone-whatsapp-setup-2026",
        "https://lightfriend.ai/blog/digital-detox-with-whatsapp",
        "https://lightfriend.ai/blog/smartphone-at-home-dumbphone",
        "https://lightfriend.ai/blog/punkt-mp02-whatsapp",
        "https://lightfriend.ai/blog/mudita-kompakt-whatsapp",
        "https://lightfriend.ai/blog/sunbeam-f1-pro-whatsapp",
        "https://lightfriend.ai/blog/nokia-2780-whatsapp",
    ] {
        assert!(
            sitemap.contains(&format!("<loc>{url}</loc>")),
            "sitemap should contain {url}"
        );
    }
}

#[test]
fn sitemap_excludes_nonexistent_and_private_routes() {
    let store = load_blog();
    let sitemap = &store.sitemap_xml;

    for url in [
        "https://lightfriend.ai/faq",
        "https://lightfriend.ai/login",
        "https://lightfriend.ai/admin",
        "https://lightfriend.ai/billing",
    ] {
        assert!(
            !sitemap.contains(&format!("<loc>{url}</loc>")),
            "sitemap should not contain {url}"
        );
    }
}

#[test]
fn blog_posts_have_index_links_and_page_metadata() {
    let store = load_blog();
    assert!(store
        .blog_index_html
        .contains("Dumbphone WhatsApp, Telegram, Signal &amp; AI Guides"));
    for path in [
        "/whatsapp-on-dumbphone",
        "/telegram-on-dumbphone",
        "/signal-on-dumbphone",
        "/ai-assistant-by-sms",
        "/can-i-leave-my-smartphone",
    ] {
        assert!(
            store.blog_index_html.contains(&format!(r#"href="{path}""#)),
            "blog index should expose the high-intent crawl path {path}"
        );
    }
    let maintained_slugs = [
        "ai-assistant-via-sms",
        "intentional-notifications",
        "message-digests",
        "cross-service-urgency-context",
        "messages-without-a-smartphone",
        "keep-whatsapp-linked-device-connected",
        "send-messages-with-a-cancel-window",
        "when-a-connected-service-is-unavailable",
        "privacy-minded-minimal-assistant",
        "what-lightfriend-does-not-try-to-do",
        "best-dumbphone-whatsapp-setup-2026",
        "digital-detox-with-whatsapp",
        "smartphone-at-home-dumbphone",
        "punkt-mp02-whatsapp",
        "mudita-kompakt-whatsapp",
        "sunbeam-f1-pro-whatsapp",
        "nokia-2780-whatsapp",
    ];

    for slug in maintained_slugs {
        let post = store.get_post(slug).expect("sorted slug should resolve");
        let canonical = format!("https://lightfriend.ai/blog/{slug}");
        assert!(
            post.full_page_html
                .contains(&format!(r#"<link rel="canonical" href="{canonical}">"#)),
            "post {slug} should contain its canonical URL"
        );
        assert!(
            post.full_page_html.contains(r#"<meta name="description""#),
            "post {slug} should contain a meta description"
        );
        assert!(
            post.full_page_html
                .contains(r#"data-fast-goal="blog_pricing_click""#),
            "post {slug} should track pricing CTA clicks"
        );
        if matches!(
            slug,
            "digital-detox-with-whatsapp" | "smartphone-at-home-dumbphone"
        ) {
            assert!(
                post.full_page_html
                    .contains(r#"href="/how-to-switch-to-dumbphone""#),
                "post {slug} should link back to the switching hub"
            );
        }
        assert!(
            store
                .blog_index_html
                .contains(&format!(r#"href="/blog/{slug}""#)),
            "blog index should link to {slug}"
        );
    }
}

#[test]
fn robots_points_to_public_sitemap_and_blocks_private_sections() {
    let robots = include_str!("../static/robots.txt");
    let production_robots = include_str!("../../frontend/robots.txt");
    assert!(robots.contains("Sitemap: https://lightfriend.ai/sitemap.xml"));
    assert!(robots.contains("User-agent: OAI-SearchBot\nAllow: /"));
    assert!(production_robots.contains("User-agent: GPTBot\nAllow: /"));
    assert!(production_robots.contains("User-agent: ClaudeBot\nAllow: /"));
    for route in ["/api/", "/admin", "/billing"] {
        assert!(
            robots.contains(&format!("Disallow: {route}")),
            "robots should disallow {route}"
        );
        assert!(
            production_robots.contains(&format!("Disallow: {route}")),
            "production robots should disallow {route}"
        );
    }
}

#[tokio::test]
async fn llms_handlers_serve_plain_text_instead_of_the_spa_fallback() {
    let response = backend::blog::handlers::llms_txt_handler().await;
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/plain; charset=utf-8"
    );
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = std::str::from_utf8(&body).unwrap();
    assert!(body.contains("Lightfriend"));
    assert!(body.contains("best-dumbphone-whatsapp-setup-2026"));

    let response = backend::blog::handlers::llms_full_txt_handler().await;
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = std::str::from_utf8(&body).unwrap();
    assert!(body.contains("Lightfriend"));
    assert!(body.contains("digital-detox-with-whatsapp"));

    let response = backend::blog::handlers::smartphone_exit_plan_md_handler().await;
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/markdown; charset=utf-8"
    );
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = std::str::from_utf8(&body).unwrap();
    assert!(body.contains("# Can I Leave My Smartphone?"));
    assert!(body.contains("The phone is not the platform"));
}
