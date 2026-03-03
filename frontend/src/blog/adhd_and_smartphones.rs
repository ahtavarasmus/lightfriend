use yew::prelude::*;
use yew_router::components::Link;
use crate::Route;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(AdhdAndSmartphones)]
pub fn adhd_and_smartphones() -> Html {
    use_seo(SeoMeta {
        title: "ADHD and Smartphones: Why Dumbphones Are the Ultimate ADHD Tool",
        description: "Discover why smartphones worsen ADHD symptoms and how switching to a dumbphone can dramatically improve focus, productivity, and mental health.",
        canonical: "https://lightfriend.ai/blog/adhd-and-smartphones",
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
                <h1>{"ADHD and Smartphones: Why Dumbphones Are the Ultimate ADHD Tool"}</h1>
                <p>{"Your smartphone is hijacking your dopamine. Here\u{2019}s how to fight back."}</p>
            </section>
            <section class="blog-content">
                <h2>{"The ADHD-Smartphone Trap"}</h2>
                <p>{"If you have ADHD, your smartphone is arguably the worst device you can carry. The ADHD brain is already wired to chase novelty and dopamine \u{2013} and smartphones are engineered to deliver exactly that in an endless stream. Notifications, infinite scrolling, short-form video, and social media algorithms are designed to exploit the exact neurological patterns that make ADHD challenging."}</p>
                <p>{"Studies show that adults with ADHD spend 30\u{2013}40% more time on their phones than neurotypical adults. The constant context-switching destroys what little sustained focus ADHD brains can muster. Every notification is a derailment that costs 15\u{2013}25 minutes to recover from."}</p>

                <h2>{"Why Smartphones Make ADHD Worse"}</h2>
                <ul>
                    <li><strong>{"Dopamine loops:"}</strong>{" Each scroll, like, and notification triggers a micro-dose of dopamine. The ADHD brain, already dopamine-deficient, becomes addicted to this easy supply instead of earning dopamine from meaningful tasks."}</li>
                    <li><strong>{"Infinite scrolling:"}</strong>{" There is no natural stopping point. For a brain that struggles with self-regulation, this is devastating. \u{201c}Just five minutes\u{201d} becomes an hour."}</li>
                    <li><strong>{"Task paralysis amplified:"}</strong>{" When you are struggling to start a task, the phone offers an instant escape. The harder the task, the more tempting the phone."}</li>
                    <li><strong>{"Sleep disruption:"}</strong>{" ADHD already impairs sleep regulation. Blue light and late-night scrolling make it dramatically worse, creating a vicious cycle of poor sleep and worsened symptoms."}</li>
                </ul>

                <h2>{"How Dumbphones Help ADHD"}</h2>
                <p>{"A dumbphone removes the problem at the source. No apps to scroll, no feeds to check, no notifications to chase. Your brain stops receiving that constant drip of cheap dopamine and starts redirecting attention to real-world activities."}</p>
                <p>{"People with ADHD who switch to dumbphones consistently report improved focus during work, better sleep, reduced anxiety, and a dramatic drop in screen time from 6\u{2013}8 hours to under 30 minutes per day."}</p>

                <h2>{"Practical Tips for the Switch"}</h2>
                <ol>
                    <li><strong>{"Start with a weekend trial."}</strong>{" Put your smartphone in a drawer for 48 hours and use only a dumbphone. Notice how your brain adjusts."}</li>
                    <li><strong>{"Set up Lightfriend first."}</strong>{" Connect your email, WhatsApp, and calendar so urgent messages still reach you via SMS."}</li>
                    <li><strong>{"Move 2FA to a YubiKey."}</strong>{" So you do not need your smartphone for authentication."}</li>
                    <li><strong>{"Keep your computer for focused work."}</strong>{" Use Cold Turkey to block distracting sites during work hours."}</li>
                    <li><strong>{"Tell people about the switch."}</strong>{" Let friends and family know your new number or that you are using Lightfriend to bridge messages."}</li>
                </ol>

                <h2>{"Lightfriend: The Missing Piece"}</h2>
                <p>{"The biggest fear with going dumbphone is missing something important. Lightfriend solves this by monitoring your email, WhatsApp, Telegram, and calendar, then forwarding only what matters to your dumbphone via SMS. You can also text or call Lightfriend\u{2019}s AI to search the web, get directions, or control smart home devices. All the utility of a smartphone, none of the distraction."}</p>

                <div class="blog-cta">
                    <h3>{"Take Back Your Focus"}</h3>
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
