use yew::prelude::*;
use yew_router::components::Link;
use crate::Route;

#[function_component(Blog)]
pub fn blog() -> Html {
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
                    <Link<Route> to={Route::LightPhone3WhatsappGuide}>
                        <img src="/assets/light-phone-3-whatsapp-integration.webp" alt="Light Phone 3 with WhatsApp via Lightfriend AI" loading="lazy" class="blog-preview-image" />
                        <h2>{"Light Phone 3 WhatsApp Guide: Enhance Your Minimalist Phone with Lightfriend"}</h2>
                        <p>{"Discover how to add WhatsApp functionality to your Light Phone 3 without compromising its minimalist design. Stay connected via SMS and voice while maintaining digital detox benefits."}</p>
                        <span class="blog-date">{"August 13, 2025"}</span>
                    </Link<Route>>
                </div>
                // Add more blog post previews here as needed
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
                    background-image: url('/assets/field_asthetic.webp');
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
