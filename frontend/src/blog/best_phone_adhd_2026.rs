use yew::prelude::*;
use yew_router::components::Link;
use crate::Route;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(BestPhoneAdhd2026)]
pub fn best_phone_adhd_2026() -> Html {
    use_seo(SeoMeta {
        title: "Best Phone for ADHD in 2026: Complete Guide",
        description: "Find the best phone for ADHD in 2026. Dumbphone recommendations, Lightfriend setup tips, and daily routines for managing ADHD with a minimalist phone.",
        canonical: "https://lightfriend.ai/blog/best-phone-for-adhd-2026",
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
                <h1>{"Best Phone for ADHD in 2026: Complete Guide"}</h1>
                <p>{"The right phone can be a game-changer for managing ADHD."}</p>
            </section>
            <section class="blog-content">
                <h2>{"Why Phone Choice Matters for ADHD"}</h2>
                <p>{"For people with ADHD, a smartphone is not just a distraction \u{2013} it is an active obstacle to functioning. The constant stream of notifications, endless apps, and algorithm-driven content directly exploit the ADHD brain\u{2019}s vulnerability to novelty-seeking and difficulty with self-regulation. Choosing the right phone is one of the highest-impact decisions you can make for managing your ADHD."}</p>

                <h2>{"Top Pick: Light Phone 3"}</h2>
                <p>{"The Light Phone 3 is the best overall phone for ADHD. Its E-ink display eliminates the dopamine-triggering visual stimulation of a color screen. It includes essential tools (directions, music, camera, alarm) without any social media, browser, or app store. The built-in hotspot lets you tether a laptop when you need internet access."}</p>
                <ul>
                    <li><strong>{"Why it is great for ADHD:"}</strong>{" No apps to get lost in, calming E-ink display, essential tools only, hotspot for laptop."}</li>
                    <li><strong>{"Price:"}</strong>{" ~$399"}</li>
                </ul>

                <h2>{"Budget Pick: Nokia 2780 Flip"}</h2>
                <p>{"If the Light Phone 3 is out of your budget, the Nokia 2780 Flip is an excellent alternative. The flip form factor adds a physical barrier to phone use \u{2013} you have to open it to see the screen. KaiOS keeps things simple with only basic apps."}</p>
                <ul>
                    <li><strong>{"Why it is great for ADHD:"}</strong>{" Physical flip barrier, simple interface, long battery (one less thing to worry about), affordable."}</li>
                    <li><strong>{"Price:"}</strong>{" ~$60"}</li>
                </ul>

                <h2>{"For the Purist: Punkt MP02"}</h2>
                <p>{"If you want the absolute minimum, the Punkt MP02 offers calls, texts, and a hotspot. Nothing else. For ADHD brains that need the most extreme reduction in phone-based temptation, this is it."}</p>
                <ul>
                    <li><strong>{"Why it is great for ADHD:"}</strong>{" Absolute minimum features, zero temptation, beautiful design that feels intentional."}</li>
                    <li><strong>{"Price:"}</strong>{" ~$350"}</li>
                </ul>

                <h2>{"Setting Up Lightfriend for ADHD"}</h2>
                <p>{"Once you have your dumbphone, set up Lightfriend to handle the things you actually need:"}</p>
                <ol>
                    <li><strong>{"Connect email"}</strong>{" and set it to forward only important messages. Less noise, less distraction."}</li>
                    <li><strong>{"Bridge WhatsApp and Telegram"}</strong>{" so you do not miss messages from friends and family."}</li>
                    <li><strong>{"Connect your calendar"}</strong>{" to receive SMS reminders. ADHD and time-blindness are closely linked \u{2013} SMS reminders are a lifeline."}</li>
                    <li><strong>{"Use the AI assistant"}</strong>{" for quick questions instead of falling into a browser rabbit hole."}</li>
                </ol>

                <h2>{"Daily Routine Tips"}</h2>
                <ul>
                    <li><strong>{"Morning:"}</strong>{" Text Lightfriend \"what\u{2019}s on my calendar today?\" for a quick daily briefing."}</li>
                    <li><strong>{"During work:"}</strong>{" Your dumbphone sits silently. Only truly important messages reach you via Lightfriend."}</li>
                    <li><strong>{"Breaks:"}</strong>{" No phone to scroll. Read a book, take a walk, or just sit with your thoughts."}</li>
                    <li><strong>{"Evening:"}</strong>{" Text Lightfriend \"any important emails?\" for a quick check, then enjoy your evening."}</li>
                    <li><strong>{"Before bed:"}</strong>{" No blue light from a smartphone screen. Your sleep quality improves, and tomorrow\u{2019}s ADHD symptoms are milder."}</li>
                </ul>

                <div class="blog-cta">
                    <h3>{"Take Control of Your ADHD"}</h3>
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
