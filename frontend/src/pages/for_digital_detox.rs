use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(ForDigitalDetox)]
pub fn for_digital_detox() -> Html {
    use_seo(SeoMeta {
        title: "Digital Detox Without Losing Messaging | Lightfriend",
        description: "Go on a digital detox without losing access to messaging, email, and calendar. Lightfriend bridges your dumbphone to the services you need.",
        canonical: "https://lightfriend.ai/for/digital-detox",
        og_type: "website",
    });

    html! {
        <div class="audience-page">
            <div class="audience-hero">
                <h1>{"Digital Detox Without Losing Messaging"}</h1>
                <p class="audience-subtitle">{"Ditch the smartphone. Keep the communication."}</p>
            </div>
            <div class="audience-content">
                <section>
                    <h2>{"What You Keep"}</h2>
                    <p>{"Switching to a dumbphone doesn\u{2019}t mean going off the grid. Lightfriend forwards the essentials to your phone via SMS:"}</p>
                    <ul>
                        <li>{"WhatsApp, Telegram, and Signal messages"}</li>
                        <li>{"Email \u{2014} read and reply via text"}</li>
                        <li>{"Calendar reminders and event summaries"}</li>
                        <li>{"AI-powered web search when you need answers"}</li>
                        <li>{"GPS navigation via voice or text"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"What You Lose"}</h2>
                    <p>{"The things that were stealing your time and attention:"}</p>
                    <ul>
                        <li>{"Social media feeds and infinite scrolling"}</li>
                        <li>{"Push notifications from dozens of apps"}</li>
                        <li>{"Compulsive screen checking"}</li>
                        <li>{"Late-night doomscrolling"}</li>
                        <li>{"App-driven anxiety and FOMO"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"How to Start Your Detox"}</h2>
                    <ol>
                        <li>{"Pick a dumbphone \u{2014} Light Phone 3, Nokia flip, or any basic phone"}</li>
                        <li>{"Sign up for Lightfriend and connect your messaging apps"}</li>
                        <li>{"Put your smartphone in a drawer"}</li>
                        <li>{"Enjoy the calm \u{2014} you\u{2019}ll still get every important message"}</li>
                    </ol>
                </section>
                <section class="audience-cta">
                    <a href="/register" class="cta-button">{"Start Your Detox"}</a>
                    <a href="/pricing" class="cta-link">{"View pricing \u{2192}"}</a>
                </section>
            </div>
        </div>
    }
}
