use backend::utils::seo_headers::should_noindex_path;

#[test]
fn wasm_bundles_are_not_indexable_search_results() {
    assert!(should_noindex_path("/frontend-eb441ce4262f8ca3_bg.wasm"));
    assert!(should_noindex_path("/assets/worker.wasm"));
}

#[test]
fn public_answer_pages_remain_indexable() {
    for path in [
        "/",
        "/blog",
        "/light-phone-3-whatsapp-guide",
        "/telegram-on-dumbphone",
        "/ai-assistant-by-sms",
    ] {
        assert!(!should_noindex_path(path), "{path} should stay indexable");
    }
}
