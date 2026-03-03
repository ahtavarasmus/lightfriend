use yew::prelude::*;
use yew_router::components::Link;
use crate::Route;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(WhatsappWithoutSmartphone)]
pub fn whatsapp_without_smartphone() -> Html {
    use_seo(SeoMeta {
        title: "How to Use WhatsApp Without a Smartphone (2026 Guide)",
        description: "All methods to use WhatsApp without a smartphone in 2026. WhatsApp Web, Lightfriend SMS bridge, KaiOS phones, and more.",
        canonical: "https://lightfriend.ai/blog/whatsapp-without-smartphone",
        og_type: "article",
    });
    {
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    window.scroll_to_with_x_and_y(0.0, 0.0);
                }
                || ()
            },
            (),
        );
    }
    html! {
        <div class="blog-page">
            <div class="blog-background"></div>
            <section class="blog-hero">
                <h1>{"How to Use WhatsApp Without a Smartphone"}</h1>
                <p>{"Stay connected on WhatsApp from any phone \u{2013} no smartphone required."}</p>
            </section>
            <section class="blog-content">
                <h2>{"The WhatsApp Problem"}</h2>
                <p>{"WhatsApp is the world\u{2019}s most popular messaging app with over 2 billion users. For many people, especially in Europe, South America, and Asia, it is the primary way to communicate with family, friends, and even businesses. This makes it the single biggest barrier to switching to a dumbphone."}</p>
                <p>{"The good news: there are several ways to keep using WhatsApp without carrying a smartphone. Here is every method available in 2026."}</p>

                <h2>{"Method 1: WhatsApp Web + Computer"}</h2>
                <p>{"WhatsApp now supports linking up to four devices without needing your phone online. You can use WhatsApp Web or the desktop app on your computer as your primary WhatsApp client."}</p>
                <ul>
                    <li><strong>{"Setup:"}</strong>{" Open web.whatsapp.com on your computer, scan the QR code with a smartphone (borrow one briefly), and your computer stays linked for up to 14 days."}</li>
                    <li><strong>{"Limitation:"}</strong>{" You need to re-link every 14 days if the phone is not online. You also need access to a computer."}</li>
                </ul>

                <h2>{"Method 2: Lightfriend SMS Bridge"}</h2>
                <p>{"Lightfriend bridges WhatsApp messages to SMS, so you receive and send WhatsApp messages directly from your dumbphone as regular text messages. This is the most seamless solution for daily use."}</p>
                <ol>
                    <li>{"Sign up for Lightfriend and connect your WhatsApp account."}</li>
                    <li>{"Incoming WhatsApp messages are forwarded to your dumbphone as SMS."}</li>
                    <li>{"Reply via SMS and your response is sent back through WhatsApp."}</li>
                    <li>{"Works with any phone that can send and receive texts."}</li>
                </ol>
                <p>{"This method works everywhere, requires no computer, and keeps you in the loop 24/7."}</p>

                <h2>{"Method 3: KaiOS Phones"}</h2>
                <p>{"Some KaiOS feature phones like the Nokia 2780 Flip and Nokia 8110 have a basic WhatsApp app available in the KaiStore. It is a stripped-down version but supports text messaging, voice messages, and photo sharing."}</p>
                <ul>
                    <li><strong>{"Pros:"}</strong>{" Native WhatsApp on a simple phone, no computer needed."}</li>
                    <li><strong>{"Cons:"}</strong>{" Typing on a T9 keyboard is slow, the app can be buggy, limited features compared to the smartphone version, and WhatsApp may drop KaiOS support in the future."}</li>
                </ul>

                <h2>{"Which Method Should You Choose?"}</h2>
                <p>{"If you are always near a computer, "}<strong>{"WhatsApp Web"}</strong>{" works well. If you want WhatsApp everywhere without a computer, "}<strong>{"Lightfriend\u{2019}s SMS bridge"}</strong>{" is the most reliable and convenient option. KaiOS is a fallback but not recommended as a long-term solution due to uncertain app support."}</p>
                <p>{"Many dumbphone users combine methods: Lightfriend for on-the-go messaging and WhatsApp Web when at their desk."}</p>

                <div class="blog-cta">
                    <h3>{"Keep WhatsApp, Ditch the Smartphone"}</h3>
                    <Link<Route> to={Route::Pricing} classes="forward-link">
                        <button class="hero-cta">{"Get Started with Lightfriend"}</button>
                    </Link<Route>>
                </div>
            </section>
            <style>
                {r#"
                .blog-page { padding-top: 74px; min-height: 100vh; color: #ffffff; position: relative; background: transparent; }
                .blog-background { position: fixed; top: 0; left: 0; width: 100%; height: 100vh; background-image: url('/assets/field_asthetic_not.webp'); background-size: cover; background-position: center; background-repeat: no-repeat; opacity: 1; z-index: -2; pointer-events: none; }
                .blog-background::after { content: ''; position: absolute; bottom: 0; left: 0; width: 100%; height: 50%; background: linear-gradient(to bottom, rgba(26, 26, 26, 0) 0%, rgba(26, 26, 26, 1) 100%); }
                .blog-hero { text-align: center; padding: 6rem 2rem; background: rgba(26, 26, 26, 0.75); backdrop-filter: blur(5px); margin-top: 2rem; border: 1px solid rgba(30, 144, 255, 0.1); margin-bottom: 2rem; }
                .blog-hero h1 { font-size: 3.5rem; margin-bottom: 1.5rem; background: linear-gradient(45deg, #fff, #7EB2FF); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
                .blog-hero p { font-size: 1.2rem; color: #999; max-width: 600px; margin: 0 auto; }
                .blog-content { max-width: 800px; margin: 0 auto; padding: 2rem; }
                .blog-content h2 { font-size: 2.5rem; margin: 3rem 0 1rem; background: linear-gradient(45deg, #fff, #7EB2FF); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
                .blog-content p { color: #999; line-height: 1.6; margin-bottom: 1.5rem; }
                .blog-content ul, .blog-content ol { color: #999; padding-left: 1.5rem; margin-bottom: 1.5rem; }
                .blog-content li { margin-bottom: 0.75rem; }
                .blog-content a { color: #7EB2FF; text-decoration: none; border-bottom: 1px solid rgba(126, 178, 255, 0.3); transition: all 0.3s ease; font-weight: 500; }
                .blog-content a:hover { color: #ffffff; border-bottom-color: #7EB2FF; text-shadow: 0 0 5px rgba(126, 178, 255, 0.5); }
                .blog-cta { text-align: center; margin: 4rem 0 2rem; padding: 2rem; background: rgba(30, 144, 255, 0.1); border-radius: 12px; }
                .blog-cta h3 { font-size: 2rem; margin-bottom: 1.5rem; background: linear-gradient(45deg, #fff, #7EB2FF); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
                .hero-cta { background: linear-gradient(45deg, #7EB2FF, #4169E1); color: white; border: none; padding: 1rem 2.5rem; border-radius: 8px; font-size: 1.1rem; cursor: pointer; transition: all 0.3s ease; }
                .hero-cta:hover { transform: translateY(-2px); box-shadow: 0 4px 20px rgba(126, 178, 255, 0.4); }
                @media (max-width: 768px) { .blog-hero { padding: 4rem 1rem; } .blog-hero h1 { font-size: 2.5rem; } .blog-content { padding: 1rem; } .blog-content h2 { font-size: 2rem; } }
                "#}
            </style>
        </div>
    }
}
