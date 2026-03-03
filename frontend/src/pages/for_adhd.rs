use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(ForAdhd)]
pub fn for_adhd() -> Html {
    use_seo(SeoMeta {
        title: "Best Phone for ADHD \u{2013} Dumbphone + AI Assistant | Lightfriend",
        description: "The best phone solution for ADHD. A dumbphone with Lightfriend removes distractions while keeping essential communication.",
        canonical: "https://lightfriend.ai/for/adhd",
        og_type: "website",
    });

    html! {
        <div class="audience-page">
            <div class="audience-hero">
                <h1>{"Best Phone for ADHD"}</h1>
                <p class="audience-subtitle">{"A dumbphone removes distractions. Lightfriend keeps you connected."}</p>
            </div>
            <div class="audience-content">
                <section>
                    <h2>{"Why a Dumbphone Helps with ADHD"}</h2>
                    <p>{"Smartphones with constant notifications, infinite scrolling, and app switching are designed to capture attention \u{2014} the exact thing ADHD makes hard to manage. A dumbphone eliminates these triggers entirely."}</p>
                    <ul>
                        <li>{"No infinite scrolling \u{2014} social media and news feeds are gone"}</li>
                        <li>{"No impulse checking \u{2014} nothing to compulsively check"}</li>
                        <li>{"Reduced context switching \u{2014} calls and texts only"}</li>
                        <li>{"Better sleep \u{2014} no blue light rabbit holes"}</li>
                        <li>{"Improved focus \u{2014} your phone stops being a distraction"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"What About Messaging, Email, Calendar?"}</h2>
                    <p>{"Lightfriend handles the digital services you need:"}</p>
                    <ul>
                        <li>{"WhatsApp, Telegram, Signal via SMS"}</li>
                        <li>{"Email access via text"}</li>
                        <li>{"Calendar reminders"}</li>
                        <li>{"AI web search"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Recommended Setup"}</h2>
                    <ol>
                        <li>{"Get a Light Phone 3 or Nokia flip phone"}</li>
                        <li>{"Sign up for Lightfriend Autopilot ($29/month)"}</li>
                        <li>{"Connect messaging apps and email"}</li>
                        <li>{"Use your computer for screen tasks with website blockers"}</li>
                    </ol>
                </section>
                <section class="audience-cta">
                    <a href="/register" class="cta-button">{"Get Started"}</a>
                    <a href="/pricing" class="cta-link">{"View pricing \u{2192}"}</a>
                </section>
            </div>
        </div>
    }
}
