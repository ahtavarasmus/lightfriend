use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(VoiceAi)]
pub fn voice_ai() -> Html {
    use_seo(SeoMeta {
        title: "Voice AI Assistant – Call AI from Any Phone | Lightfriend",
        description: "Call an AI assistant from any phone. Get answers, search the web, and access your integrations via voice call from Light Phone, Nokia, or any basic phone.",
        canonical: "https://lightfriend.ai/features/voice-ai",
        og_type: "website",
    });

    html! {
        <div class="feature-page">
            <div class="feature-hero">
                <h1>{"Voice AI Assistant"}</h1>
                <p class="feature-subtitle">{"Call an AI assistant from any phone — get answers and help via voice"}</p>
            </div>
            <div class="feature-content">
                <section>
                    <h2>{"How It Works"}</h2>
                    <p>{"Voice assistants like Siri and Google Assistant require a smartphone. With Lightfriend, you can call an AI assistant from any phone — just dial your Lightfriend number and talk naturally."}</p>
                    <p>{"Lightfriend uses advanced voice AI powered by ElevenLabs to understand your questions and respond conversationally. Ask anything you'd ask a search engine, and get a spoken answer."}</p>
                </section>
                <section>
                    <h2>{"Features"}</h2>
                    <ul>
                        <li>{"Natural voice conversation with AI"}</li>
                        <li>{"Web search via voice"}</li>
                        <li>{"Access all Lightfriend integrations by voice"}</li>
                        <li>{"Check messages, calendar, and email"}</li>
                        <li>{"Control Tesla and smart home devices"}</li>
                        <li>{"Works from any phone with calling"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Compatible Phones"}</h2>
                    <p>{"Works with Light Phone 2 & 3, Nokia flip phones, any basic phone, any flip phone, landlines — any phone that can make calls."}</p>
                </section>
                <section class="feature-cta">
                    <a href="/register" class="cta-button">{"Get Voice AI on Your Phone"}</a>
                    <a href="/pricing" class="cta-link">{"View pricing →"}</a>
                </section>
            </div>
        </div>
    }
}
