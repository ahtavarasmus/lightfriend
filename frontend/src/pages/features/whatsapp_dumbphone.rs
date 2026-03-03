use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(WhatsAppDumbphone)]
pub fn whatsapp_dumbphone() -> Html {
    use_seo(SeoMeta {
        title: "WhatsApp on Dumbphone – Use WhatsApp Without a Smartphone | Lightfriend",
        description: "Use WhatsApp on any dumbphone or flip phone via SMS. Send, receive, and monitor WhatsApp messages from Light Phone, Nokia, or any basic phone.",
        canonical: "https://lightfriend.ai/features/whatsapp-dumbphone",
        og_type: "website",
    });

    html! {
        <div class="feature-page">
            <div class="feature-hero">
                <h1>{"WhatsApp on Dumbphone"}</h1>
                <p class="feature-subtitle">{"Use WhatsApp without a smartphone — send and receive messages via SMS"}</p>
            </div>
            <div class="feature-content">
                <section>
                    <h2>{"How It Works"}</h2>
                    <p>{"WhatsApp is the world's most popular messaging app with over 2 billion users. With Lightfriend, you can send and receive WhatsApp messages via SMS from any phone — no smartphone needed."}</p>
                    <p>{"Lightfriend bridges WhatsApp and SMS. Incoming WhatsApp messages are forwarded as text messages. Reply by texting Lightfriend, and your message is sent via WhatsApp."}</p>
                </section>
                <section>
                    <h2>{"Features"}</h2>
                    <ul>
                        <li>{"Send messages to any WhatsApp contact"}</li>
                        <li>{"Receive incoming WhatsApp messages as SMS"}</li>
                        <li>{"Get notifications for important messages"}</li>
                        <li>{"Access WhatsApp groups"}</li>
                        <li>{"24/7 monitoring with Autopilot plan"}</li>
                        <li>{"Daily digest summaries"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Compatible Phones"}</h2>
                    <p>{"Works with Light Phone 2 & 3, Nokia flip phones, any basic phone, any flip phone — any phone that can send SMS."}</p>
                </section>
                <section class="feature-cta">
                    <a href="/register" class="cta-button">{"Get WhatsApp on Your Dumbphone"}</a>
                    <a href="/pricing" class="cta-link">{"View pricing →"}</a>
                </section>
            </div>
        </div>
    }
}
