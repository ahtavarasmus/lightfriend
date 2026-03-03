use yew::prelude::*;
use yew_router::components::Link;
use crate::Route;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(LightfriendVsBeeper)]
pub fn lightfriend_vs_beeper() -> Html {
    use_seo(SeoMeta {
        title: "Lightfriend vs Beeper vs Bridge Apps: Which Messaging Solution?",
        description: "Detailed comparison of Lightfriend, Beeper, and Matrix bridge apps for unified messaging. Features, pricing, pros and cons for dumbphone users.",
        canonical: "https://lightfriend.ai/blog/lightfriend-vs-beeper",
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
                <h1>{"Lightfriend vs Beeper vs Bridge Apps"}</h1>
                <p>{"Which unified messaging solution is right for you?"}</p>
            </section>
            <section class="blog-content">
                <h2>{"The Messaging Fragmentation Problem"}</h2>
                <p>{"Your friends are on WhatsApp, your family on iMessage, your coworkers on Slack, and that one group on Telegram. Managing multiple messaging platforms is exhausting on a smartphone \u{2013} and nearly impossible on a dumbphone. Several solutions aim to unify your messages, but they work very differently."}</p>

                <h2>{"Beeper (Automattic)"}</h2>
                <p>{"Beeper, now owned by Automattic, is a universal chat app that brings all your messaging platforms into one interface. It runs on smartphones, desktops, and the web."}</p>
                <ul>
                    <li><strong>{"Supported platforms:"}</strong>{" WhatsApp, Telegram, Signal, Slack, Discord, Instagram DMs, Facebook Messenger, Google Chat, and more."}</li>
                    <li><strong>{"Pros:"}</strong>{" Beautiful unified interface, search across all platforms, free to use, strong privacy stance."}</li>
                    <li><strong>{"Cons:"}</strong>{" Requires a smartphone or computer \u{2013} does not work on dumbphones. You still need a screen to interact with messages."}</li>
                </ul>

                <h2>{"Matrix Bridges (Self-Hosted)"}</h2>
                <p>{"Matrix is an open-source messaging protocol. You can self-host a Matrix server and set up bridges to connect WhatsApp, Telegram, Signal, and other platforms."}</p>
                <ul>
                    <li><strong>{"Pros:"}</strong>{" Full control over your data, free (minus server costs), highly customizable, open source."}</li>
                    <li><strong>{"Cons:"}</strong>{" Requires significant technical knowledge to set up and maintain. Bridges can break when platforms change their APIs. No SMS integration out of the box."}</li>
                </ul>

                <h2>{"Lightfriend"}</h2>
                <p>{"Lightfriend bridges your messaging platforms to SMS and voice, making them accessible from any phone \u{2013} including dumbphones. It also includes an AI assistant for web search, email, calendar, and smart home control."}</p>
                <ul>
                    <li><strong>{"Supported platforms:"}</strong>{" WhatsApp, Telegram, Signal, email (Gmail/Outlook), calendar, Tesla, Home Assistant, and more."}</li>
                    <li><strong>{"Pros:"}</strong>{" Works on any phone with SMS, AI assistant included, no technical setup required, managed service."}</li>
                    <li><strong>{"Cons:"}</strong>{" Monthly subscription, SMS character limits for long messages, slight latency compared to native apps."}</li>
                </ul>

                <h2>{"Comparison at a Glance"}</h2>
                <table class="comparison-table">
                    <tr>
                        <th>{"Feature"}</th>
                        <th>{"Lightfriend"}</th>
                        <th>{"Beeper"}</th>
                        <th>{"Matrix Bridges"}</th>
                    </tr>
                    <tr>
                        <td>{"Works on dumbphones"}</td>
                        <td>{"Yes"}</td>
                        <td>{"No"}</td>
                        <td>{"No"}</td>
                    </tr>
                    <tr>
                        <td>{"Setup difficulty"}</td>
                        <td>{"Easy"}</td>
                        <td>{"Easy"}</td>
                        <td>{"Hard"}</td>
                    </tr>
                    <tr>
                        <td>{"AI assistant"}</td>
                        <td>{"Yes"}</td>
                        <td>{"No"}</td>
                        <td>{"No"}</td>
                    </tr>
                    <tr>
                        <td>{"Smart home control"}</td>
                        <td>{"Yes"}</td>
                        <td>{"No"}</td>
                        <td>{"No"}</td>
                    </tr>
                    <tr>
                        <td>{"Self-hosted option"}</td>
                        <td>{"No"}</td>
                        <td>{"No"}</td>
                        <td>{"Yes"}</td>
                    </tr>
                    <tr>
                        <td>{"Price"}</td>
                        <td>{"From $5/mo"}</td>
                        <td>{"Free"}</td>
                        <td>{"Server costs"}</td>
                    </tr>
                </table>

                <h2>{"Which Should You Choose?"}</h2>
                <p>{"If you use a dumbphone or want to, "}<strong>{"Lightfriend"}</strong>{" is the only option that bridges messaging to SMS. If you use a smartphone but want unified messaging, "}<strong>{"Beeper"}</strong>{" is excellent. If you are technically skilled and want full control, "}<strong>{"Matrix bridges"}</strong>{" give you maximum flexibility. Many people use Beeper on their computer and Lightfriend on their dumbphone \u{2013} the two complement each other well."}</p>

                <div class="blog-cta">
                    <h3>{"Try Lightfriend Today"}</h3>
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
                .comparison-table { width: 100%; border-collapse: collapse; margin: 2rem 0; color: #ddd; }
                .comparison-table th, .comparison-table td { padding: 1rem; border: 1px solid rgba(126, 178, 255, 0.2); text-align: left; }
                .comparison-table th { background: rgba(0, 0, 0, 0.5); color: #7EB2FF; }
                .blog-cta { text-align: center; margin: 4rem 0 2rem; padding: 2rem; background: rgba(30, 144, 255, 0.1); border-radius: 12px; }
                .blog-cta h3 { font-size: 2rem; margin-bottom: 1.5rem; background: linear-gradient(45deg, #fff, #7EB2FF); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
                .hero-cta { background: linear-gradient(45deg, #7EB2FF, #4169E1); color: white; border: none; padding: 1rem 2.5rem; border-radius: 8px; font-size: 1.1rem; cursor: pointer; transition: all 0.3s ease; }
                .hero-cta:hover { transform: translateY(-2px); box-shadow: 0 4px 20px rgba(126, 178, 255, 0.4); }
                @media (max-width: 768px) { .blog-hero { padding: 4rem 1rem; } .blog-hero h1 { font-size: 2.5rem; } .blog-content { padding: 1rem; } .blog-content h2 { font-size: 2rem; } .comparison-table th, .comparison-table td { padding: 0.75rem; font-size: 0.9rem; } }
                "#}
            </style>
        </div>
    }
}
