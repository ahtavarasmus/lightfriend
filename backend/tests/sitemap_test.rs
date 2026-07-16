use backend::blog::content::BlogStore;

#[test]
fn sitemap_contains_indexable_public_routes_and_blog_posts() {
    let store = BlogStore::load("content/blog").expect("blog content should load");
    let sitemap = &store.sitemap_xml;

    for url in [
        "https://lightfriend.ai/",
        "https://lightfriend.ai/pricing",
        "https://lightfriend.ai/bring-own-number",
        "https://lightfriend.ai/blog",
        "https://lightfriend.ai/blog/ai-assistant-via-sms",
    ] {
        assert!(
            sitemap.contains(&format!("<loc>{url}</loc>")),
            "sitemap should contain {url}"
        );
    }
}

#[test]
fn sitemap_excludes_nonexistent_and_private_routes() {
    let store = BlogStore::load("content/blog").expect("blog content should load");
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
