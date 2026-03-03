use yew::prelude::*;
use yew_router::components::Link;
use crate::Route;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(DigitalDetoxGuide)]
pub fn digital_detox_guide() -> Html {
    use_seo(SeoMeta {
        title: "Digital Detox Guide: Everything You Need to Know in 2026",
        description: "The complete digital detox guide for 2026. Benefits, step-by-step instructions, tools, and how to stay connected during your detox.",
        canonical: "https://lightfriend.ai/blog/digital-detox-guide",
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
                <h1>{"Digital Detox Guide: Everything You Need to Know in 2026"}</h1>
                <p>{"Reclaim your attention, sleep, and mental health with a structured digital detox."}</p>
            </section>
            <section class="blog-content">
                <h2>{"What Is a Digital Detox?"}</h2>
                <p>{"A digital detox is a deliberate period of reducing or eliminating the use of digital devices \u{2013} particularly smartphones and social media. It is not about going off-grid permanently. It is about resetting your relationship with technology so that you control it, rather than the other way around."}</p>

                <h2>{"Proven Benefits"}</h2>
                <ul>
                    <li><strong>{"Better sleep:"}</strong>{" Reduced blue light exposure and late-night scrolling leads to faster sleep onset and deeper rest."}</li>
                    <li><strong>{"Improved focus:"}</strong>{" Without constant notifications, your ability to concentrate on single tasks improves dramatically within days."}</li>
                    <li><strong>{"Lower anxiety:"}</strong>{" Social media comparison and news doomscrolling are major anxiety triggers. Removing them brings measurable relief."}</li>
                    <li><strong>{"Deeper relationships:"}</strong>{" When you are not checking your phone during conversations, your relationships improve."}</li>
                    <li><strong>{"More free time:"}</strong>{" The average person spends 3\u{2013}7 hours daily on their phone. Reclaiming even half of that is life-changing."}</li>
                </ul>

                <h2>{"Step-by-Step Digital Detox Plan"}</h2>
                <ol>
                    <li><strong>{"Audit your screen time."}</strong>{" Check your phone\u{2019}s screen time settings. Know your baseline before you start."}</li>
                    <li><strong>{"Set clear goals."}</strong>{" Decide what you are detoxing from (social media, news, all apps?) and for how long (weekend, week, month, permanent?)."}</li>
                    <li><strong>{"Notify important contacts."}</strong>{" Tell family, friends, and colleagues how to reach you during the detox."}</li>
                    <li><strong>{"Set up alternatives."}</strong>{" Get a dumbphone, set up Lightfriend for essential communications, install Cold Turkey on your computer."}</li>
                    <li><strong>{"Remove temptations."}</strong>{" Put your smartphone in a drawer, delete apps, or give it to a trusted friend."}</li>
                    <li><strong>{"Fill the void."}</strong>{" Plan activities: books, exercise, cooking, hobbies. Boredom is the biggest risk for relapse."}</li>
                    <li><strong>{"Reflect and adjust."}</strong>{" After your detox period, decide what to bring back and what to leave behind permanently."}</li>
                </ol>

                <h2>{"Tools for Your Detox"}</h2>
                <ul>
                    <li><strong>{"Dumbphone:"}</strong>{" A Light Phone, Nokia flip, or any basic phone for calls and texts."}</li>
                    <li><strong>{"Lightfriend:"}</strong>{" Bridges your essential digital services (email, WhatsApp, calendar) to SMS so you stay reachable without a smartphone."}</li>
                    <li><strong>{"Cold Turkey:"}</strong>{" Blocks distracting websites and apps on your computer."}</li>
                    <li><strong>{"Physical books and notebooks:"}</strong>{" Replace digital reading and note-taking with analog alternatives."}</li>
                </ul>

                <h2>{"Staying Connected During Detox"}</h2>
                <p>{"The number one reason people fail at digital detoxes is the fear of missing important messages. Lightfriend eliminates this by forwarding urgent emails, WhatsApp messages, and calendar reminders to your dumbphone via SMS. You can also call or text the Lightfriend AI assistant to search the web, check the weather, or get directions. You stay connected to what matters while disconnecting from what does not."}</p>

                <div class="blog-cta">
                    <h3>{"Start Your Digital Detox Today"}</h3>
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
