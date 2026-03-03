use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(Autopilot)]
pub fn autopilot() -> Html {
    use_seo(SeoMeta {
        title: "Autopilot: Proactive Monitoring – 24/7 Message Monitoring | Lightfriend",
        description: "Autopilot monitors your messages 24/7 and alerts you to important ones via SMS. Never miss urgent WhatsApp, Telegram, Signal, or email messages on your dumbphone.",
        canonical: "https://lightfriend.ai/features/autopilot",
        og_type: "website",
    });

    html! {
        <div class="feature-page">
            <div class="feature-hero">
                <h1>{"Autopilot: Proactive Monitoring"}</h1>
                <p class="feature-subtitle">{"24/7 message monitoring — get alerted to important messages automatically"}</p>
            </div>
            <div class="feature-content">
                <section>
                    <h2>{"How It Works"}</h2>
                    <p>{"Autopilot monitors your connected messaging platforms around the clock and uses AI to identify important messages. When something urgent arrives, you get an SMS alert immediately — no need to check manually."}</p>
                    <p>{"Configure your alert preferences and let Lightfriend handle the rest. Get daily digest summaries, urgent message alerts, or both. Autopilot ensures you never miss what matters."}</p>
                </section>
                <section>
                    <h2>{"Features"}</h2>
                    <ul>
                        <li>{"24/7 monitoring of WhatsApp, Telegram, Signal, and email"}</li>
                        <li>{"AI-powered importance detection"}</li>
                        <li>{"Instant SMS alerts for urgent messages"}</li>
                        <li>{"Daily and weekly digest summaries"}</li>
                        <li>{"Customizable alert rules and keywords"}</li>
                        <li>{"Quiet hours and do-not-disturb settings"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Compatible Phones"}</h2>
                    <p>{"Works with Light Phone 2 & 3, Nokia flip phones, any basic phone, any flip phone — any phone that can receive SMS."}</p>
                </section>
                <section class="feature-cta">
                    <a href="/register" class="cta-button">{"Enable Autopilot Monitoring"}</a>
                    <a href="/pricing" class="cta-link">{"View pricing →"}</a>
                </section>
            </div>
        </div>
    }
}
