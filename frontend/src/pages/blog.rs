use yew::prelude::*;
use yew_router::components::Link;
use crate::Route;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(Blog)]
pub fn blog() -> Html {
    use_seo(SeoMeta {
        title: "Blog \u{2013} Lightfriend Guides & Minimalist Living Tips",
        description: "Guides, tips, and insights on minimalist living with a dumbphone. Learn how to switch to a Light Phone, read more, and stay connected without a smartphone.",
        canonical: "https://lightfriend.ai/blog",
        og_type: "website",
    });
    // Scroll to top only on initial mount
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
        <div class="blog-list-page">
            <div class="blog-list-background"></div>
            <section class="blog-list-hero">
                <h1>{"Blog"}</h1>
                <p>{"Latest updates, guides, and insights on minimalist living with Lightfriend"}</p>
            </section>
            <section class="blog-list-section">

                <div class="blog-post-preview">
                    <Link<Route> to={Route::ReadMoreAccidentallyGuide}>
                        <img src="/assets/man_accidentally_reading_books.png" alt="How to Read Books Accidentally" loading="lazy" class="blog-preview-image" />
                        <h2>{"How to Read Books Accidentally"}</h2>
                        <p>{"How to Read More Without Willpower"}</p>
                        <span class="blog-date">{"August 21, 2025"}</span>
                    </Link<Route>>
                </div>
                <div class="blog-post-preview">
                    <Link<Route> to={Route::SwitchToDumbphoneGuide}>
                        <img src="/assets/lightphone2.png" alt="How to Switch to a Dumbphone" loading="lazy" class="blog-preview-image" />
                        <h2>{"How to Switch to a Dumbphone"}</h2>
                        <p>{"All the things you need to consider when joining the dumbphone revolution."}</p>
                        <span class="blog-date">{"August 19, 2025"}</span>
                    </Link<Route>>
                </div>
                <div class="blog-post-preview">
                    <Link<Route> to={Route::LightPhone3WhatsappGuide}>
                        <img src="/assets/light-phone-3-whatsapp-integration.webp" alt="Light Phone 3 with WhatsApp via Lightfriend AI" loading="lazy" class="blog-preview-image" />
                        <h2>{"Light Phone 3 WhatsApp Guide: Enhance Your Minimalist Phone with Lightfriend"}</h2>
                        <p>{"Discover how to add WhatsApp functionality to your Light Phone 3 without compromising its minimalist design. Stay connected via SMS and voice while maintaining digital detox benefits."}</p>
                        <span class="blog-date">{"August 13, 2025"}</span>
                    </Link<Route>>

                </div>

                // New blog posts - March 2026
                <div class="blog-post-preview">
                    <Link<Route> to={Route::BlogBestDumbphones}>
                        <h2>{"Best Dumbphones in 2026: Complete Buyer\u{2019}s Guide"}</h2>
                        <p>{"The definitive guide to Light Phone 3, Nokia flip phones, Punkt MP02, and more. Find the perfect minimalist phone."}</p>
                        <span class="blog-date">{"March 3, 2026"}</span>
                    </Link<Route>>
                </div>
                <div class="blog-post-preview">
                    <Link<Route> to={Route::BlogAdhdSmartphones}>
                        <h2>{"ADHD and Smartphones: Why Dumbphones Are the Ultimate ADHD Tool"}</h2>
                        <p>{"How smartphones exploit ADHD vulnerabilities, and why switching to a dumbphone can transform focus and mental health."}</p>
                        <span class="blog-date">{"March 3, 2026"}</span>
                    </Link<Route>>
                </div>
                <div class="blog-post-preview">
                    <Link<Route> to={Route::BlogWhatsappWithout}>
                        <h2>{"How to Use WhatsApp Without a Smartphone"}</h2>
                        <p>{"All the methods to access WhatsApp on a dumbphone, flip phone, or feature phone in 2026."}</p>
                        <span class="blog-date">{"March 3, 2026"}</span>
                    </Link<Route>>
                </div>
                <div class="blog-post-preview">
                    <Link<Route> to={Route::BlogDigitalDetox}>
                        <h2>{"Digital Detox Guide: Everything You Need to Know"}</h2>
                        <p>{"A comprehensive guide to digital detox \u{2013} benefits, step-by-step plan, and how to stay connected without a smartphone."}</p>
                        <span class="blog-date">{"March 3, 2026"}</span>
                    </Link<Route>>
                </div>
                <div class="blog-post-preview">
                    <Link<Route> to={Route::BlogTeslaSms}>
                        <h2>{"Tesla Control via SMS: Manage Your Tesla Without a Smartphone"}</h2>
                        <p>{"Lock, unlock, climate control, battery check \u{2013} all Tesla commands you can send via text message."}</p>
                        <span class="blog-date">{"March 3, 2026"}</span>
                    </Link<Route>>
                </div>
                <div class="blog-post-preview">
                    <Link<Route> to={Route::BlogVsBeeper}>
                        <h2>{"Lightfriend vs Beeper vs Bridge Apps: Which Messaging Solution?"}</h2>
                        <p>{"Detailed comparison of unified messaging solutions for dumbphone users."}</p>
                        <span class="blog-date">{"March 3, 2026"}</span>
                    </Link<Route>>
                </div>
                <div class="blog-post-preview">
                    <Link<Route> to={Route::BlogBestAi}>
                        <h2>{"Best AI Assistants in 2026: Complete Comparison"}</h2>
                        <p>{"Siri, Google Assistant, Alexa, ChatGPT, Claude, Lightfriend \u{2013} which work on dumbphones?"}</p>
                        <span class="blog-date">{"March 3, 2026"}</span>
                    </Link<Route>>
                </div>
                <div class="blog-post-preview">
                    <Link<Route> to={Route::BlogEmailDumbphone}>
                        <h2>{"How to Get Email on a Dumbphone"}</h2>
                        <p>{"Gmail and Outlook access from any basic phone. Forwarding, bridges, and the best method."}</p>
                        <span class="blog-date">{"March 3, 2026"}</span>
                    </Link<Route>>
                </div>
                <div class="blog-post-preview">
                    <Link<Route> to={Route::BlogHomeAssistant}>
                        <h2>{"Home Assistant via SMS: Control Your Smart Home from Any Phone"}</h2>
                        <p>{"Lights, thermostat, locks \u{2013} control everything through text messages."}</p>
                        <span class="blog-date">{"March 3, 2026"}</span>
                    </Link<Route>>
                </div>
                <div class="blog-post-preview">
                    <Link<Route> to={Route::BlogScanQr}>
                        <h2>{"How to Scan QR Codes Without a Smartphone"}</h2>
                        <p>{"QR codes are everywhere. Here\u{2019}s how dumbphone users can decode them."}</p>
                        <span class="blog-date">{"March 3, 2026"}</span>
                    </Link<Route>>
                </div>
                <div class="blog-post-preview">
                    <Link<Route> to={Route::BlogBestPhoneAdhd}>
                        <h2>{"Best Phone for ADHD in 2026"}</h2>
                        <p>{"Phone recommendations, setup guides, and daily routines for managing ADHD with a dumbphone."}</p>
                        <span class="blog-date">{"March 3, 2026"}</span>
                    </Link<Route>>
                </div>
                <div class="blog-post-preview">
                    <Link<Route> to={Route::BlogTelegramSignal}>
                        <h2>{"How to Use Telegram and Signal Without a Smartphone"}</h2>
                        <p>{"Access secure messaging apps from any basic phone via SMS."}</p>
                        <span class="blog-date">{"March 3, 2026"}</span>
                    </Link<Route>>
                </div>
            </section>
            <style>
                {r#"
                .blog-list-page {
                    padding-top: 74px;
                    min-height: 100vh;
                    color: #ffffff;
                    position: relative;
                    background: transparent;
                }
                .blog-list-background {
                    position: fixed;
                    top: 0;
                    left: 0;
                    width: 100%;
                    height: 100vh;
                    background-image: url('/assets/rain.gif');
                    background-size: cover;
                    background-position: center;
                    background-repeat: no-repeat;
                    opacity: 1;
                    z-index: -2;
                    pointer-events: none;
                }
                .blog-list-background::after {
                    content: '';
                    position: absolute;
                    bottom: 0;
                    left: 0;
                    width: 100%;
                    height: 50%;
                    background: linear-gradient(
                        to bottom,
                        rgba(26, 26, 26, 0) 0%,
                        rgba(26, 26, 26, 1) 100%
                    );
                }
                .blog-list-hero {
                    text-align: center;
                    padding: 6rem 2rem;
                    background: rgba(26, 26, 26, 0.75);
                    backdrop-filter: blur(5px);
                    margin-top: 2rem;
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    margin-bottom: 2rem;
                }
                .blog-list-hero h1 {
                    font-size: 3.5rem;
                    margin-bottom: 1.5rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }
                .blog-list-hero p {
                    font-size: 1.2rem;
                    color: #999;
                    max-width: 600px;
                    margin: 0 auto;
                }
                .blog-list-section {
                    max-width: 800px;
                    margin: 0 auto;
                    padding: 2rem;
                }
                .blog-post-preview {
                    background: rgba(26, 26, 26, 0.85);
                    backdrop-filter: blur(10px);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 12px;
                    margin-bottom: 2rem;
                    overflow: hidden;
                    transition: all 0.3s ease;
                }
                .blog-post-preview:hover {
                    border-color: rgba(30, 144, 255, 0.3);
                    transform: translateY(-5px);
                }
                .blog-post-preview a {
                    text-decoration: none;
                    color: inherit;
                    display: block;
                }
                .blog-preview-image {
                    width: 100%;
                    height: auto;
                    display: block;
                }
                .blog-post-preview h2 {
                    font-size: 1.8rem;
                    padding: 1.5rem 1.5rem 0;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }
                .blog-post-preview p {
                    color: #999;
                    padding: 0 1.5rem;
                    margin: 1rem 0;
                }
                .blog-date {
                    display: block;
                    padding: 0 1.5rem 1.5rem;
                    color: #666;
                    font-size: 0.9rem;
                }
                @media (max-width: 768px) {
                    .blog-list-hero {
                        padding: 4rem 1rem;
                    }
                    .blog-list-hero h1 {
                        font-size: 2.5rem;
                    }
                    .blog-list-section {
                        padding: 1rem;
                    }
                    .blog-post-preview h2 {
                        font-size: 1.5rem;
                    }
                }
                "#}
            </style>
        </div>
    }
}
