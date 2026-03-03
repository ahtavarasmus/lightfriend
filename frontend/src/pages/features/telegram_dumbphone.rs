use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(TelegramDumbphone)]
pub fn telegram_dumbphone() -> Html {
    use_seo(SeoMeta {
        title: "Telegram on Dumbphone – Use Telegram Without a Smartphone | Lightfriend",
        description: "Use Telegram on any dumbphone or flip phone via SMS. Send, receive, and monitor Telegram messages from Light Phone, Nokia, or any basic phone.",
        canonical: "https://lightfriend.ai/features/telegram-dumbphone",
        og_type: "website",
    });

    html! {
        <div class="feature-page">
            <div class="feature-hero">
                <h1>{"Telegram on Dumbphone"}</h1>
                <p class="feature-subtitle">{"Access Telegram without a smartphone — send and receive messages via SMS"}</p>
            </div>
            <div class="feature-content">
                <section>
                    <h2>{"How It Works"}</h2>
                    <p>{"Telegram is a fast, secure messaging platform used by hundreds of millions worldwide. With Lightfriend, you can send and receive Telegram messages via SMS from any phone — no smartphone or internet connection needed."}</p>
                    <p>{"Lightfriend bridges Telegram and SMS. Incoming Telegram messages are forwarded to your phone as text messages. Reply by texting Lightfriend, and your response is delivered through Telegram."}</p>
                </section>
                <section>
                    <h2>{"Features"}</h2>
                    <ul>
                        <li>{"Send messages to any Telegram contact"}</li>
                        <li>{"Receive incoming Telegram messages as SMS"}</li>
                        <li>{"Get notifications for important messages"}</li>
                        <li>{"Access Telegram groups and channels"}</li>
                        <li>{"24/7 monitoring with Autopilot plan"}</li>
                        <li>{"Daily digest summaries"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Compatible Phones"}</h2>
                    <p>{"Works with Light Phone 2 & 3, Nokia flip phones, any basic phone, any flip phone — any phone that can send SMS."}</p>
                </section>
                <section class="feature-cta">
                    <a href="/register" class="cta-button">{"Get Telegram on Your Dumbphone"}</a>
                    <a href="/pricing" class="cta-link">{"View pricing →"}</a>
                </section>
            </div>
        </div>
    }
}
