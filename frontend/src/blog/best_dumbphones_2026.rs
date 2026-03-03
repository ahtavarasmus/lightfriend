use yew::prelude::*;
use yew_router::components::Link;
use crate::Route;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(BestDumbphones2026)]
pub fn best_dumbphones_2026() -> Html {
    use_seo(SeoMeta {
        title: "Best Dumbphones in 2026: Complete Buyer\u{2019}s Guide",
        description: "The definitive guide to the best dumbphones and minimalist phones in 2026. Light Phone 3, Nokia flip phones, Punkt, and more.",
        canonical: "https://lightfriend.ai/blog/best-dumbphones-2026",
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
                <h1>{"Best Dumbphones in 2026: Complete Buyer\u{2019}s Guide"}</h1>
                <p>{"Finding the perfect minimalist phone for a distraction-free life."}</p>
            </section>
            <section class="blog-content">
                <h2>{"Why Go Dumb in 2026?"}</h2>
                <p>{"Dumbphones have gone from a niche curiosity to a mainstream movement. With screen time averages exceeding 7 hours per day, more people than ever are choosing minimalist phones to reclaim their attention, mental health, and time. Here are the best options available right now."}</p>

                <h2>{"1. Light Phone 3"}</h2>
                <p>{"The Light Phone 3 is the crown jewel of the minimalist phone world. It features a beautiful E-ink display, a capable camera, Wi-Fi hotspot, and a carefully curated set of tools including directions, music, podcasts, and an alarm."}</p>
                <ul>
                    <li><strong>{"Pros:"}</strong>{" Gorgeous design, E-ink display easy on the eyes, hotspot for laptop tethering, active development team adding new tools."}</li>
                    <li><strong>{"Cons:"}</strong>{" Premium price point (~$399), E-ink refresh rate not ideal for maps, US-centric availability."}</li>
                </ul>

                <h2>{"2. Light Phone 2"}</h2>
                <p>{"The predecessor that started the movement. Still a solid choice with its compact form factor, E-ink screen, and essential tools. Now available at a lower price."}</p>
                <ul>
                    <li><strong>{"Pros:"}</strong>{" Proven reliability, lower price than LP3, strong community, hotspot support."}</li>
                    <li><strong>{"Cons:"}</strong>{" No camera, 4G only (no 5G), smaller screen, slower processor."}</li>
                </ul>

                <h2>{"3. Nokia 2780 Flip"}</h2>
                <p>{"Nokia\u{2019}s modern take on the classic flip phone. KaiOS-powered with basic apps, Google Assistant, and solid build quality. Great for those who want a familiar form factor."}</p>
                <ul>
                    <li><strong>{"Pros:"}</strong>{" Affordable (~$60), satisfying flip mechanism, FM radio, long battery life, Google Maps."}</li>
                    <li><strong>{"Cons:"}</strong>{" KaiOS can be sluggish, small screen, basic camera, limited app selection."}</li>
                </ul>

                <h2>{"4. Nokia 2760 Flip"}</h2>
                <p>{"The budget-friendly sibling of the 2780. Runs KaiOS with essential features at an even lower price point."}</p>
                <ul>
                    <li><strong>{"Pros:"}</strong>{" Very affordable (~$40), compact, decent battery, basic WhatsApp via KaiOS."}</li>
                    <li><strong>{"Cons:"}</strong>{" Lower build quality, less RAM, slower performance, limited storage."}</li>
                </ul>

                <h2>{"5. Punkt MP02"}</h2>
                <p>{"The Swiss-designed minimalist phone for purists. Designed by Jasper Morrison, it focuses purely on calls and texts with a premium build and integrated Signal messaging."}</p>
                <ul>
                    <li><strong>{"Pros:"}</strong>{" Beautiful industrial design, Signal integration for encrypted messaging, Wi-Fi hotspot, excellent call quality."}</li>
                    <li><strong>{"Cons:"}</strong>{" Expensive (~$350), no camera, no maps, very limited feature set, occasional firmware bugs."}</li>
                </ul>

                <h2>{"6. CAT B35"}</h2>
                <p>{"The rugged option for outdoor enthusiasts and tradespeople. Military-grade durability with KaiOS, built to survive drops, dust, and water."}</p>
                <ul>
                    <li><strong>{"Pros:"}</strong>{" IP68 waterproof, MIL-STD-810H rated, loud speaker, long battery, affordable (~$80)."}</li>
                    <li><strong>{"Cons:"}</strong>{" Bulky, basic KaiOS experience, heavy, not the most stylish."}</li>
                </ul>

                <h2>{"Which One Is Right for You?"}</h2>
                <p>{"If you want the best overall experience and don\u{2019}t mind paying a premium, the "}<strong>{"Light Phone 3"}</strong>{" is the clear winner. For budget-conscious buyers, the "}<strong>{"Nokia 2780 Flip"}</strong>{" offers the best value. If you work outdoors or need durability, the "}<strong>{"CAT B35"}</strong>{" is your pick. And if minimalist design is your priority, the "}<strong>{"Punkt MP02"}</strong>{" is a work of art."}</p>
                <p>{"No matter which dumbphone you choose, pair it with "}<strong>{"Lightfriend"}</strong>{" to keep access to email, messaging apps, AI assistance, and smart home control \u{2013} all through simple SMS and voice calls. You get the calm of a dumbphone without sacrificing your digital life."}</p>

                <div class="blog-cta">
                    <h3>{"Ready to Go Dumb?"}</h3>
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
