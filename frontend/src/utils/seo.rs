use yew::prelude::*;

pub struct SeoMeta {
    pub title: &'static str,
    pub description: &'static str,
    pub canonical: &'static str,
    pub og_type: &'static str,
}

/// Sets page-specific title, meta description, canonical URL, and Open Graph tags.
/// Restores the default (index.html) values on unmount.
#[hook]
pub fn use_seo(meta: SeoMeta) {
    use_effect_with_deps(
        move |_| {
            if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                // Set title
                document.set_title(meta.title);

                // Helper to set or create a meta tag
                let set_meta = |selector: &str, attr: &str, value: &str| {
                    if let Ok(Some(el)) = document.query_selector(selector) {
                        el.set_attribute(attr, value).ok();
                    }
                };

                set_meta(
                    r#"meta[name="description"]"#,
                    "content",
                    meta.description,
                );
                set_meta(
                    r#"meta[property="og:title"]"#,
                    "content",
                    meta.title,
                );
                set_meta(
                    r#"meta[property="og:description"]"#,
                    "content",
                    meta.description,
                );
                set_meta(
                    r#"meta[property="og:url"]"#,
                    "content",
                    meta.canonical,
                );
                set_meta(
                    r#"meta[property="og:type"]"#,
                    "content",
                    meta.og_type,
                );
                set_meta(
                    r#"meta[name="twitter:title"]"#,
                    "content",
                    meta.title,
                );
                set_meta(
                    r#"meta[name="twitter:description"]"#,
                    "content",
                    meta.description,
                );

                // Update canonical link
                if let Ok(Some(el)) = document.query_selector(r#"link[rel="canonical"]"#) {
                    el.set_attribute("href", meta.canonical).ok();
                }
            }

            // Cleanup: restore defaults on unmount
            || {
                if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                    doc.set_title("Lightfriend: The Best AI Assistant for Dumbphones \u{2013} WhatsApp, Telegram, Signal, Email & More");
                    let restore = |selector: &str, attr: &str, value: &str| {
                        if let Ok(Some(el)) = doc.query_selector(selector) {
                            el.set_attribute(attr, value).ok();
                        }
                    };
                    restore(r#"meta[name="description"]"#, "content", "Lightfriend: AI assistant for dumbphones like Light Phone 3, Nokia flip phones, and other minimalist phones. Access WhatsApp, Telegram, Signal, email, calendar, AI search, and GPS via SMS/voice. Enhance your digital detox without unwanted isolation.");
                    restore(r#"meta[property="og:title"]"#, "content", "lightfriend: AI Assistant for Dumbphones & Minimalist Phones like Light Phone 3");
                    restore(r#"meta[property="og:description"]"#, "content", "Enhance any dumbphone with WhatsApp, Telegram, Signal, email, and more via AI. Perfect match for The Light Phone or Nokia flip phones. Stay connected without apps or screens.");
                    restore(r#"meta[property="og:url"]"#, "content", "https://lightfriend.ai");
                    restore(r#"meta[property="og:type"]"#, "content", "website");
                    restore(r#"meta[name="twitter:title"]"#, "content", "lightfriend: A Proactive AI Assistant for Dumbphone");
                    restore(r#"meta[name="twitter:description"]"#, "content", "Enhance any dumbphone with WhatsApp, Telegram, email, and more via AI. Perfect match for The Light Phone or Nokia flip phones. Stay connected without apps or screens.");
                    restore(r#"link[rel="canonical"]"#, "href", "https://lightfriend.ai");
                }
            }
        },
        (),
    );
}
