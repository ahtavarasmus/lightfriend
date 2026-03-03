use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(SignalDumbphone)]
pub fn signal_dumbphone() -> Html {
    use_seo(SeoMeta {
        title: "Signal on Dumbphone – Use Signal Without a Smartphone | Lightfriend",
        description: "Use Signal on any dumbphone or flip phone via SMS. Send, receive, and monitor Signal messages from Light Phone, Nokia, or any basic phone.",
        canonical: "https://lightfriend.ai/features/signal-dumbphone",
        og_type: "website",
    });

    html! {
        <div class="feature-page">
            <div class="feature-hero">
                <h1>{"Signal on Dumbphone"}</h1>
                <p class="feature-subtitle">{"Use Signal's secure messaging without a smartphone — via SMS"}</p>
            </div>
            <div class="feature-content">
                <section>
                    <h2>{"How It Works"}</h2>
                    <p>{"Signal is the gold standard for private messaging, trusted by privacy advocates worldwide. With Lightfriend, you can send and receive Signal messages via SMS from any phone — no smartphone required."}</p>
                    <p>{"Lightfriend bridges Signal and SMS. Incoming Signal messages are forwarded to your phone as text messages. Reply by texting Lightfriend, and your response is delivered through Signal."}</p>
                </section>
                <section>
                    <h2>{"Features"}</h2>
                    <ul>
                        <li>{"Send messages to any Signal contact"}</li>
                        <li>{"Receive incoming Signal messages as SMS"}</li>
                        <li>{"Get notifications for important messages"}</li>
                        <li>{"Access Signal group conversations"}</li>
                        <li>{"24/7 monitoring with Autopilot plan"}</li>
                        <li>{"Daily digest summaries"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Compatible Phones"}</h2>
                    <p>{"Works with Light Phone 2 & 3, Nokia flip phones, any basic phone, any flip phone — any phone that can send SMS."}</p>
                </section>
                <section class="feature-cta">
                    <a href="/register" class="cta-button">{"Get Signal on Your Dumbphone"}</a>
                    <a href="/pricing" class="cta-link">{"View pricing →"}</a>
                </section>
            </div>
        </div>
    }
}
